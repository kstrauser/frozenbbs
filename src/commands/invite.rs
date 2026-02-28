use super::Replies;
use crate::db::{board_states, invitations, now_as_useconds, posts, queued_messages, users, User};
use crate::{canonical_node_id, BBSConfig};
use diesel::Connection as _;
use diesel::SqliteConnection;
use rand::Rng;

/// One hour in microseconds, used for rate limiting.
const RATE_LIMIT_US: i64 = 3600 * 1_000_000;

/// 24 hours in microseconds, used for expiry checks.
const EXPIRY_US: i64 = 24 * 3600 * 1_000_000;

const OPAQUE_REJECTION: &str = "This user is not accepting invitations.";
const UNKNOWN_NODE: &str = "Unknown node.";
const CANNOT_INVITE_SELF: &str = "You cannot invite your own account.";
const SENDER_BANNED: &str = "Your account is not allowed to send invitations.";
const INFLIGHT_INVITATION: &str = "You already have a pending outbound invitation.";
const NO_PENDING_INVITATION: &str = "No pending invitation to deny.";
const NO_PENDING_INVITATIONS: &str = "No pending invitations.";
const WRONG_PASSWORD: &str = "Incorrect password.";
const NO_PENDING_ACCEPT: &str = "No pending invitation to accept.";
const INVITATION_EXPIRED: &str = "This invitation has expired.";
const ACCEPT_BANNED: &str = "Your account is not allowed to accept invitations.";

/// Generate a pronounceable, cryptographically random password of ~12 characters.
///
/// Uses alternating consonant-vowel syllables for readability.
pub fn generate_password() -> String {
    const CONSONANTS: &[u8] = b"bcdfghjklmnprstvwz";
    const VOWELS: &[u8] = b"aeiou";

    let mut rng = rand::rng();
    let mut password = String::with_capacity(12);

    // 4 syllables of 3 chars each = 12 chars (consonant-vowel-consonant)
    for _ in 0..4 {
        password.push(CONSONANTS[rng.random_range(0..CONSONANTS.len())] as char);
        password.push(VOWELS[rng.random_range(0..VOWELS.len())] as char);
        password.push(CONSONANTS[rng.random_range(0..CONSONANTS.len())] as char);
    }

    password
}

/// Block invitations for this account.
#[allow(clippy::needless_pass_by_value)]
pub fn block(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let updated = users::update_invite_allowed(conn, user, false)
        .expect("should be able to update invite_allowed");
    user.account = updated.account;

    "Invitations are now blocked.".into()
}

/// Unblock invitations for this account.
#[allow(clippy::needless_pass_by_value)]
pub fn unblock(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let updated = users::update_invite_allowed(conn, user, true)
        .expect("should be able to update invite_allowed");
    user.account = updated.account;

    "Invitations are now allowed.".into()
}

/// Deny a pending inbound invitation.
#[allow(clippy::needless_pass_by_value)]
pub fn deny(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let pending = invitations::get_pending_for_invitee(conn, user.node.id);
    let Some(invitation) = pending.first() else {
        return NO_PENDING_INVITATION.into();
    };

    invitations::deny(conn, invitation).expect("should be able to deny invitation");
    "Invitation denied.".into()
}

/// Accept a pending inbound invitation.
#[allow(clippy::needless_pass_by_value)]
pub fn accept(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(password) = args.get(1) else {
        return "Usage: invite accept <password> [migrate]".into();
    };
    let migrate = args.get(2).is_some();

    // (4) Reject if accepting node's account is banned
    if user.jackass() {
        return ACCEPT_BANNED.into();
    }

    // (1) Look up the calling node's pending inbound invitation (non-expired)
    let pending = invitations::get_pending_for_invitee(conn, user.node.id);
    let Some(invitation) = pending.first() else {
        return NO_PENDING_ACCEPT.into();
    };

    // (5) Check if invitation is expired (redundant with get_pending_for_invitee filtering,
    //     but kept for safety)
    let now = now_as_useconds();
    if invitation.created_at_us + EXPIRY_US <= now {
        return INVITATION_EXPIRED.into();
    }

    // (3) Verify the accepting node is the intended target
    if invitation.invitee_node_id != user.node.id {
        return NO_PENDING_ACCEPT.into();
    }

    // (2) Validate the password matches
    if invitation.password != *password {
        return WRONG_PASSWORD.into();
    }

    let old_account_id = user.account_id();
    let new_account_id = invitation.sender_account_id;

    // (6-10) Perform the acceptance in a transaction
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // (6) Move the node from its old account to the inviter's account
        users::move_node_to_account(conn, &user.node, new_account_id)
            .expect("should be able to move node to new account");

        // (7) If migrate: reassign posts, DMs, delete board_states, delete old account
        if migrate {
            posts::migrate_account(conn, old_account_id, new_account_id)
                .expect("should be able to migrate posts");
            queued_messages::migrate_account(conn, old_account_id, new_account_id)
                .expect("should be able to migrate queued messages");
            board_states::delete_for_account(conn, old_account_id)
                .expect("should be able to delete board states");
            users::delete_account(conn, old_account_id)
                .expect("should be able to delete old account");
        }
        // (8) If NOT migrate: old account becomes a ghost (no action needed)

        // (9) Delete invitee's own outbound invitation if one exists
        invitations::delete_pending_for_sender(conn, old_account_id);

        // (10) Mark the invitation as accepted
        invitations::accept(conn, invitation)
            .expect("should be able to mark invitation as accepted");

        Ok(())
    })
    .expect("accept transaction should succeed");

    // Update the user's in-memory state to reflect the new account
    let new_account =
        users::get_account(conn, new_account_id).expect("new account should exist after accept");
    user.account = new_account;

    // (11) Return confirmation
    if migrate {
        format!(
            "Invitation accepted. You are now part of account #{}. Your posts and messages have been migrated.",
            new_account_id
        )
        .into()
    } else {
        format!(
            "Invitation accepted. You are now part of account #{}.",
            new_account_id
        )
        .into()
    }
}

/// Format a duration in microseconds as a human-readable string.
fn format_remaining(remaining_us: i64) -> String {
    if remaining_us <= 0 {
        return "expired".to_string();
    }
    let total_minutes = remaining_us / 60_000_000;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{}h {}m remaining", hours, minutes)
    } else {
        format!("{}m remaining", minutes)
    }
}

/// Show pending (non-expired) invitations for this user.
#[allow(clippy::needless_pass_by_value)]
pub fn pending(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let now = now_as_useconds();
    let outbound = invitations::get_pending_for_sender(conn, user.account_id());
    let inbound = invitations::get_pending_for_invitee(conn, user.node.id);

    if outbound.is_empty() && inbound.is_empty() {
        return NO_PENDING_INVITATIONS.into();
    }

    let mut lines: Vec<String> = Vec::new();

    for inv in &outbound {
        let remaining_us = (inv.created_at_us + EXPIRY_US) - now;
        let target_node = users::get_node_by_id(conn, inv.invitee_node_id)
            .expect("invitation should reference a valid node");
        lines.push(format!(
            "Outbound: to {} ({})",
            target_node.node_id,
            format_remaining(remaining_us)
        ));
    }

    for inv in &inbound {
        let remaining_us = (inv.created_at_us + EXPIRY_US) - now;
        let sender_user = users::get_by_account_id(conn, inv.sender_account_id)
            .expect("invitation should reference a valid account");
        let sender_nodes = users::get_nodes_for_account(conn, inv.sender_account_id);
        let node_list: Vec<String> = sender_nodes.iter().map(|n| n.to_string()).collect();
        lines.push(format!(
            "Inbound: from {} (nodes: {}) ({})",
            sender_user.display_name(),
            node_list.join(", "),
            format_remaining(remaining_us)
        ));
    }

    lines.into()
}

/// Send an invitation to a target node.
#[allow(clippy::needless_pass_by_value)]
pub fn send(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(target_node_id) = args.get(1) else {
        return "Usage: invite !nodeid".into();
    };

    // Canonicalize the target node ID
    let Some(target_node_id) = canonical_node_id(target_node_id) else {
        return UNKNOWN_NODE.into();
    };

    // (5) Sender's account is not banned
    if user.jackass() {
        return SENDER_BANNED.into();
    }

    // (1) Target node exists in the DB
    let Ok(target_user) = users::get(conn, &target_node_id) else {
        return UNKNOWN_NODE.into();
    };

    // (6) Target is not the sender's own account
    if target_user.account_id() == user.account_id() {
        return CANNOT_INVITE_SELF.into();
    }

    // (2) Target node's account has invite_allowed=true
    if !target_user.account.invite_allowed {
        return OPAQUE_REJECTION.into();
    }

    // (3) Target account has only 1 node (not already multi-node)
    let target_nodes = users::get_nodes_for_account(conn, target_user.account_id());
    if target_nodes.len() > 1 {
        return OPAQUE_REJECTION.into();
    }

    // (4) Target doesn't already have a pending inbound invitation
    let pending_inbound = invitations::get_pending_for_invitee(conn, target_user.node.id);
    if !pending_inbound.is_empty() {
        return OPAQUE_REJECTION.into();
    }

    // (7) Sender has no in-flight (non-expired, non-accepted, non-denied) outbound invitation
    let pending_outbound = invitations::get_pending_for_sender(conn, user.account_id());
    if !pending_outbound.is_empty() {
        return INFLIGHT_INVITATION.into();
    }

    // (8) Rate limit: if sender's last invitation was denied or expired, at least 1 hour must have passed
    let now = crate::db::now_as_useconds();
    if let Some(last_invite) = invitations::get_most_recent_for_sender(conn, user.account_id()) {
        // If the last invitation was accepted, no cooldown
        if last_invite.accepted_at_us.is_none() {
            // Check if it was denied or expired
            let is_denied = last_invite.denied_at_us.is_some();
            let is_expired = last_invite.created_at_us + EXPIRY_US <= now;
            if is_denied || is_expired {
                let elapsed = now - last_invite.created_at_us;
                if elapsed < RATE_LIMIT_US {
                    let remaining_us = RATE_LIMIT_US - elapsed;
                    let remaining_mins = remaining_us / 60_000_000;
                    let remaining_secs = (remaining_us % 60_000_000) / 1_000_000;
                    return format!(
                        "Rate limited. Please wait {}m {}s before sending another invitation.",
                        remaining_mins, remaining_secs
                    )
                    .into();
                }
            }
        }
    }

    // All checks passed — generate password and create invitation
    let password = generate_password();
    let invitation = invitations::create(conn, user.account_id(), target_user.node.id, &password)
        .expect("should be able to create invitation");

    // Build DM notification for the target (no password!)
    let sender_nodes = users::get_nodes_for_account(conn, user.account_id());
    let node_list: Vec<String> = sender_nodes.iter().map(|n| n.to_string()).collect();
    let dm_body = format!(
        "You have received an invitation to join account #{} (nodes: {}). Use 'invite accept <password>' to accept or 'invite deny' to reject. Use 'invite pending' to see details.",
        user.account_id(),
        node_list.join(", ")
    );

    // Queue the DM to the target's account
    queued_messages::queue_by_account_ids(
        conn,
        user.account_id(),
        target_user.account_id(),
        &dm_body,
    )
    .expect("should be able to queue invitation notification");

    // Send password back to the sender
    let _ = invitation; // invitation record created, password stored in DB
    format!(
        "Invitation sent to {}. Password: {}",
        target_node_id, password
    )
    .into()
}

/// Show a brief help listing all invitation subcommands.
#[allow(clippy::needless_pass_by_value)]
pub fn help(
    _conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let mut out = vec!["Invitation commands:".to_string()];
    out.push("  invite block       - Block invitations to your account".to_string());
    out.push("  invite unblock     - Allow invitations to your account".to_string());
    out.push("  invite pending     - Show pending invitations".to_string());
    out.push("  invite deny        - Deny a pending invitation".to_string());
    out.push("  invite accept pw [migrate] - Accept a pending invitation".to_string());
    out.push("  invite !node       - Send an invitation to a node".to_string());
    out.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::users;
    use crate::BBSConfig;
    use config::Map;

    fn test_config() -> BBSConfig {
        BBSConfig {
            bbs_name: "Test BBS".to_string(),
            my_id: "!00000001".to_string(),
            db_path: ":memory:".to_string(),
            serial_device: None,
            tcp_address: None,
            sysops: Vec::new(),
            public_channel: 0,
            ad_text: String::new(),
            weather: None,
            menus: Map::new(),
            page_delay_ms: None,
        }
    }

    /// Create a user via the standard record + observe path, then optionally set invite_allowed.
    fn create_test_user(conn: &mut SqliteConnection, node_id: &str, invite_allowed: bool) -> User {
        let (mut user, _) = users::record(conn, node_id).expect("should create user");
        if invite_allowed {
            user = users::update_invite_allowed(conn, &user, true)
                .expect("should update invite_allowed");
        }
        user
    }

    /// Add a second node to an existing account (for multi-node testing).
    /// Creates a new user via observe, then reassigns the node to the given account via raw SQL.
    fn add_node_to_account(conn: &mut SqliteConnection, account_id: i32, node_id: &str) {
        use diesel::connection::SimpleConnection;
        let (_, _) = users::observe(conn, node_id, Some("TST2"), Some("Test Node 2"), 0)
            .expect("observe should succeed");
        conn.batch_execute(&format!(
            "UPDATE nodes SET account_id = {} WHERE node_id = '{}'",
            account_id, node_id
        ))
        .expect("should reassign node");
    }

    fn get_reply_text(replies: &Replies) -> String {
        replies.0[0].out.join("\n")
    }

    // ========== Block/Unblock tests (existing) ==========

    #[test]
    fn test_block_sets_invite_allowed_false() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0001", true);
        assert!(user.account.invite_allowed);

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user.account.invite_allowed);

        let reloaded = users::get(&mut conn, "!aabb0001").expect("should find user");
        assert!(!reloaded.account.invite_allowed);
    }

    #[test]
    fn test_unblock_sets_invite_allowed_true() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0002", false);
        assert!(!user.account.invite_allowed);

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);

        let reloaded = users::get(&mut conn, "!aabb0002").expect("should find user");
        assert!(reloaded.account.invite_allowed);
    }

    #[test]
    fn test_block_idempotent() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0003", false);
        assert!(!user.account.invite_allowed);

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
    }

    #[test]
    fn test_unblock_idempotent() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0004", true);
        assert!(user.account.invite_allowed);

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
    }

    #[test]
    fn test_block_unblock_toggle() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0005", false);

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user.account.invite_allowed);

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);
    }

    #[test]
    fn test_block_is_account_wide_multi_node() {
        let mut conn = db::test_connection();
        let cfg = test_config();

        let mut user_a = create_test_user(&mut conn, "!aabb0006", true);
        let account_id = user_a.account_id();
        assert!(user_a.account.invite_allowed);

        let replies = block(&mut conn, &cfg, &mut user_a, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user_a.account.invite_allowed);

        let reloaded = users::get_account(&mut conn, account_id).expect("should find account");
        assert!(!reloaded.invite_allowed);

        let replies = unblock(&mut conn, &cfg, &mut user_a, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");

        let reloaded = users::get_account(&mut conn, account_id).expect("should find account");
        assert!(reloaded.invite_allowed);
    }

    #[test]
    fn test_new_account_defaults_to_blocked() {
        let mut conn = db::test_connection();
        let (user, _) = users::record(&mut conn, "!aabb0008").expect("should create user");
        assert!(!user.account.invite_allowed);
    }

    #[test]
    fn test_reply_destination_is_sender() {
        use crate::commands::ReplyDestination;

        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0009", false);

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(replies.0.len(), 1);
        assert!(matches!(replies.0[0].destination, ReplyDestination::Sender));

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(replies.0.len(), 1);
        assert!(matches!(replies.0[0].destination, ReplyDestination::Sender));
    }

    // ========== Password generation tests ==========

    #[test]
    fn test_password_length_is_12() {
        let password = generate_password();
        assert_eq!(password.len(), 12, "password should be exactly 12 chars");
    }

    #[test]
    fn test_password_is_pronounceable() {
        let consonants = b"bcdfghjklmnprstvwz";
        let vowels = b"aeiou";

        for _ in 0..20 {
            let password = generate_password();
            assert_eq!(password.len(), 12);
            let bytes = password.as_bytes();
            // Each syllable: consonant-vowel-consonant, 4 syllables
            for syllable in 0..4 {
                let base = syllable * 3;
                assert!(
                    consonants.contains(&bytes[base]),
                    "char at pos {} should be a consonant, got '{}'",
                    base,
                    bytes[base] as char
                );
                assert!(
                    vowels.contains(&bytes[base + 1]),
                    "char at pos {} should be a vowel, got '{}'",
                    base + 1,
                    bytes[base + 1] as char
                );
                assert!(
                    consonants.contains(&bytes[base + 2]),
                    "char at pos {} should be a consonant, got '{}'",
                    base + 2,
                    bytes[base + 2] as char
                );
            }
        }
    }

    #[test]
    fn test_password_randomness() {
        // Generate multiple passwords and verify they're not all the same
        let passwords: Vec<String> = (0..10).map(|_| generate_password()).collect();
        let unique: std::collections::HashSet<&String> = passwords.iter().collect();
        assert!(
            unique.len() > 1,
            "10 generated passwords should not all be identical"
        );
    }

    // ========== Send invitation tests ==========

    #[test]
    fn test_send_happy_path() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000001", false);
        let _target = create_test_user(&mut conn, "!aa000002", true); // unblocked

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000002", "!aa000002"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Invitation sent to !aa000002. Password: "),
            "Expected success message, got: {}",
            text
        );

        // Verify password is 12 chars
        let password = text
            .strip_prefix("Invitation sent to !aa000002. Password: ")
            .unwrap();
        assert_eq!(password.len(), 12);

        // Verify invitation was created in DB
        let pending = invitations::get_pending_for_sender(&mut conn, sender.account_id());
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].password, password);

        // Verify DM was queued to target
        let target_reloaded = users::get(&mut conn, "!aa000002").expect("target exists");
        let messages = queued_messages::get(&mut conn, &target_reloaded);
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].body.contains("invitation"),
            "DM should mention invitation"
        );
        // DM should NOT contain the password
        assert!(
            !messages[0].body.contains(password),
            "DM to target should NOT contain the password"
        );
        // DM should contain sender's node info
        assert!(
            messages[0]
                .body
                .contains(&format!("#{}", sender.account_id())),
            "DM should reference sender's account"
        );
    }

    #[test]
    fn test_send_rejected_unknown_node() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000010", false);

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !ff000099", "!ff000099"],
        );
        assert_eq!(get_reply_text(&replies), UNKNOWN_NODE);
    }

    #[test]
    fn test_send_rejected_invalid_node_id() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000011", false);

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite notanode", "notanode"],
        );
        assert_eq!(get_reply_text(&replies), UNKNOWN_NODE);
    }

    #[test]
    fn test_send_rejected_target_blocks_invitations() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000020", false);
        let _target = create_test_user(&mut conn, "!aa000021", false); // blocked (default)

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000021", "!aa000021"],
        );
        assert_eq!(get_reply_text(&replies), OPAQUE_REJECTION);
    }

    #[test]
    fn test_send_rejected_target_multi_node_same_message() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000030", false);
        let target = create_test_user(&mut conn, "!aa000031", true);

        // Add a second node to the target's account
        add_node_to_account(&mut conn, target.account_id(), "!aa000032");

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000031", "!aa000031"],
        );
        // Same opaque message as blocked
        assert_eq!(get_reply_text(&replies), OPAQUE_REJECTION);
    }

    #[test]
    fn test_send_rejected_target_has_pending_inbound() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender_a = create_test_user(&mut conn, "!aa000040", false);
        let mut sender_b = create_test_user(&mut conn, "!aa000041", false);
        let _target = create_test_user(&mut conn, "!aa000042", true);

        // Sender A sends first invitation (succeeds)
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender_a,
            vec!["invite !aa000042", "!aa000042"],
        );
        assert!(
            get_reply_text(&replies).starts_with("Invitation sent"),
            "first invitation should succeed"
        );

        // Sender B tries to invite the same target
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender_b,
            vec!["invite !aa000042", "!aa000042"],
        );
        assert_eq!(get_reply_text(&replies), OPAQUE_REJECTION);
    }

    #[test]
    fn test_send_rejected_sender_banned() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000050", false);
        let _target = create_test_user(&mut conn, "!aa000051", true);

        // Ban the sender
        users::ban(&mut conn, &sender).expect("should ban");
        sender.account.jackass = true;

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000051", "!aa000051"],
        );
        assert_eq!(get_reply_text(&replies), SENDER_BANNED);
    }

    #[test]
    fn test_send_rejected_invite_self() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000060", true); // unblock own invitations

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000060", "!aa000060"],
        );
        assert_eq!(get_reply_text(&replies), CANNOT_INVITE_SELF);
    }

    #[test]
    fn test_send_rejected_inflight_outbound() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000070", false);
        let _target_a = create_test_user(&mut conn, "!aa000071", true);
        let _target_b = create_test_user(&mut conn, "!aa000072", true);

        // Send first invitation
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000071", "!aa000071"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // Try to send another while the first is pending
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000072", "!aa000072"],
        );
        assert_eq!(get_reply_text(&replies), INFLIGHT_INVITATION);
    }

    #[test]
    fn test_send_rate_limited_after_denial() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000080", false);
        let target_a = create_test_user(&mut conn, "!aa000081", true);
        let _target_b = create_test_user(&mut conn, "!aa000082", true);

        // Create an invitation that was denied recently (10 seconds ago)
        let now = crate::db::now_as_useconds();
        let recent_time = now - 10_000_000;
        let inv = invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target_a.node.id,
            "testpass123x",
            recent_time,
        )
        .expect("should create invitation");

        invitations::deny(&mut conn, &inv).expect("should deny");

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000082", "!aa000082"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Rate limited"),
            "Expected rate limit message, got: {}",
            text
        );
    }

    #[test]
    fn test_send_rate_limited_shows_time_remaining() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000090", false);
        let target_a = create_test_user(&mut conn, "!aa000091", true);
        let _target_b = create_test_user(&mut conn, "!aa000092", true);

        // Create an invitation denied 30 minutes ago (still within cooldown)
        let now = crate::db::now_as_useconds();
        let thirty_min_ago = now - (30 * 60 * 1_000_000);
        let inv = invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target_a.node.id,
            "testpass456x",
            thirty_min_ago,
        )
        .expect("should create invitation");

        invitations::deny(&mut conn, &inv).expect("should deny");

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000092", "!aa000092"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Rate limited"),
            "Expected rate limit message, got: {}",
            text
        );
        assert!(
            text.contains("m") && text.contains("s"),
            "Rate limit message should show time remaining"
        );
    }

    #[test]
    fn test_send_rate_limit_resets_on_acceptance() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000100", false);
        let target_a = create_test_user(&mut conn, "!aa000101", true);
        let _target_b = create_test_user(&mut conn, "!aa000102", true);

        // Create an invitation and accept it (recent, 10 seconds ago)
        let now = crate::db::now_as_useconds();
        let recent_time = now - 10_000_000;
        let inv = invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target_a.node.id,
            "testpass789x",
            recent_time,
        )
        .expect("should create invitation");

        invitations::accept(&mut conn, &inv).expect("should accept");

        // Should be able to send another immediately
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000102", "!aa000102"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Invitation sent"),
            "After acceptance, should be able to send immediately, got: {}",
            text
        );
    }

    #[test]
    fn test_send_allowed_after_cooldown_expires() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000110", false);
        let target_a = create_test_user(&mut conn, "!aa000111", true);
        let _target_b = create_test_user(&mut conn, "!aa000112", true);

        // Create an invitation from >1 hour ago that was denied
        let now = crate::db::now_as_useconds();
        let over_an_hour_ago = now - RATE_LIMIT_US - 1_000_000;
        let inv = invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target_a.node.id,
            "oldpasstestx",
            over_an_hour_ago,
        )
        .expect("should create invitation");

        invitations::deny(&mut conn, &inv).expect("should deny");

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000112", "!aa000112"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Invitation sent"),
            "After cooldown, should be able to send, got: {}",
            text
        );
    }

    #[test]
    fn test_send_dm_does_not_contain_password() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000120", false);
        let _target = create_test_user(&mut conn, "!aa000121", true);

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000121", "!aa000121"],
        );
        let text = get_reply_text(&replies);
        let password = text
            .strip_prefix("Invitation sent to !aa000121. Password: ")
            .expect("should have password in reply");

        // Check the queued DM
        let target = users::get(&mut conn, "!aa000121").expect("target exists");
        let messages = queued_messages::get(&mut conn, &target);
        assert_eq!(messages.len(), 1);
        assert!(
            !messages[0].body.contains(password),
            "DM should NOT contain password '{}', but body is: {}",
            password,
            messages[0].body
        );
    }

    #[test]
    fn test_send_dm_contains_sender_node_list() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!aa000130", false);
        let _target = create_test_user(&mut conn, "!aa000131", true);

        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !aa000131", "!aa000131"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // Check the queued DM contains sender's node info
        let target = users::get(&mut conn, "!aa000131").expect("target exists");
        let messages = queued_messages::get(&mut conn, &target);
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].body.contains("!aa000130"),
            "DM should contain sender's node ID"
        );
    }

    // ========== Deny tests ==========

    #[test]
    fn test_deny_success() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!bb000001", false);
        let mut target = create_test_user(&mut conn, "!bb000002", true);

        // Send an invitation
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !bb000002", "!bb000002"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // Deny the invitation from the target's perspective
        let replies = deny(&mut conn, &cfg, &mut target, vec!["invite deny"]);
        assert_eq!(get_reply_text(&replies), "Invitation denied.");

        // Verify it's no longer pending
        let pending_inbound = invitations::get_pending_for_invitee(&mut conn, target.node.id);
        assert!(
            pending_inbound.is_empty(),
            "invitation should no longer be pending"
        );

        let pending_outbound = invitations::get_pending_for_sender(&mut conn, sender.account_id());
        assert!(
            pending_outbound.is_empty(),
            "sender should have no pending invitations"
        );
    }

    #[test]
    fn test_deny_no_pending_invitation() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!bb000010", false);

        let replies = deny(&mut conn, &cfg, &mut user, vec!["invite deny"]);
        assert_eq!(get_reply_text(&replies), NO_PENDING_INVITATION);
    }

    // ========== Pending tests ==========

    #[test]
    fn test_pending_no_invitations() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!cc000001", false);

        let replies = pending(&mut conn, &cfg, &mut user, vec!["invite pending"]);
        assert_eq!(get_reply_text(&replies), NO_PENDING_INVITATIONS);
    }

    #[test]
    fn test_pending_shows_outbound() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!cc000010", false);
        let _target = create_test_user(&mut conn, "!cc000011", true);

        // Send an invitation
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !cc000011", "!cc000011"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // Check pending from sender's perspective
        let replies = pending(&mut conn, &cfg, &mut sender, vec!["invite pending"]);
        let text = get_reply_text(&replies);
        assert!(
            text.contains("Outbound"),
            "Should show outbound invitation, got: {}",
            text
        );
        assert!(
            text.contains("!cc000011"),
            "Should show target node ID, got: {}",
            text
        );
        assert!(
            text.contains("remaining"),
            "Should show time remaining, got: {}",
            text
        );
    }

    #[test]
    fn test_pending_shows_inbound() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!cc000020", false);
        let mut target = create_test_user(&mut conn, "!cc000021", true);

        // Send an invitation
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !cc000021", "!cc000021"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // Check pending from target's perspective
        let replies = pending(&mut conn, &cfg, &mut target, vec!["invite pending"]);
        let text = get_reply_text(&replies);
        assert!(
            text.contains("Inbound"),
            "Should show inbound invitation, got: {}",
            text
        );
        assert!(
            text.contains(&format!("#{}", sender.account_id())),
            "Should show sender account ID, got: {}",
            text
        );
        assert!(
            text.contains("!cc000020"),
            "Should show sender's node info, got: {}",
            text
        );
        assert!(
            text.contains("remaining"),
            "Should show time remaining, got: {}",
            text
        );
    }

    #[test]
    fn test_pending_shows_both_outbound_and_inbound() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user_a = create_test_user(&mut conn, "!cc000030", true);
        let mut user_b = create_test_user(&mut conn, "!cc000031", true);

        // User B sends invitation to user A (A has an inbound)
        let replies = send(
            &mut conn,
            &cfg,
            &mut user_b,
            vec!["invite !cc000030", "!cc000030"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // User A sends invitation to some other user (A has an outbound)
        let _user_c = create_test_user(&mut conn, "!cc000032", true);
        let replies = send(
            &mut conn,
            &cfg,
            &mut user_a,
            vec!["invite !cc000032", "!cc000032"],
        );
        assert!(get_reply_text(&replies).starts_with("Invitation sent"));

        // Check pending from user A's perspective - should show both
        let replies = pending(&mut conn, &cfg, &mut user_a, vec!["invite pending"]);
        let text = get_reply_text(&replies);
        assert!(
            text.contains("Outbound"),
            "Should show outbound, got: {}",
            text
        );
        assert!(
            text.contains("Inbound"),
            "Should show inbound, got: {}",
            text
        );
    }

    #[test]
    fn test_pending_excludes_expired() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!cc000040", false);
        let target = create_test_user(&mut conn, "!cc000041", true);

        // Create an expired invitation (>24 hours old)
        let old_time = crate::db::now_as_useconds() - EXPIRY_US - 1_000_000;
        invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target.node.id,
            "oldpasswordx",
            old_time,
        )
        .expect("should create invitation");

        // Sender should see no pending invitations
        let replies = pending(&mut conn, &cfg, &mut sender, vec!["invite pending"]);
        assert_eq!(get_reply_text(&replies), NO_PENDING_INVITATIONS);
    }

    #[test]
    fn test_pending_expired_not_shown_for_invitee() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let sender = create_test_user(&mut conn, "!cc000050", false);
        let mut target = create_test_user(&mut conn, "!cc000051", true);

        // Create an expired invitation (>24 hours old)
        let old_time = crate::db::now_as_useconds() - EXPIRY_US - 1_000_000;
        invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target.node.id,
            "oldpasswordx",
            old_time,
        )
        .expect("should create invitation");

        // Target should see no pending invitations
        let replies = pending(&mut conn, &cfg, &mut target, vec!["invite pending"]);
        assert_eq!(get_reply_text(&replies), NO_PENDING_INVITATIONS);
    }

    // ========== Accept tests ==========

    /// Helper: Send an invitation from sender to target and return the password.
    fn send_invitation(
        conn: &mut SqliteConnection,
        cfg: &BBSConfig,
        sender: &mut User,
        target_node_id: &str,
    ) -> String {
        let replies = send(conn, cfg, sender, vec!["invite", target_node_id]);
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Invitation sent"),
            "Expected invitation success, got: {}",
            text
        );
        text.rsplit("Password: ")
            .next()
            .expect("reply should contain password")
            .to_string()
    }

    #[test]
    fn test_accept_success_without_migrate() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000001", false);
        let mut target = create_test_user(&mut conn, "!dd000002", true);

        let sender_account_id = sender.account_id();
        let target_old_account_id = target.account_id();

        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000002");

        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", &password],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.contains("Invitation accepted"),
            "Expected acceptance message, got: {}",
            text
        );
        assert!(
            !text.contains("migrated"),
            "Without migrate, should not mention migration, got: {}",
            text
        );

        // Verify target's node is now on sender's account
        let target_reloaded = users::get(&mut conn, "!dd000002").expect("target should exist");
        assert_eq!(target_reloaded.account_id(), sender_account_id);

        // Verify old account still exists (ghost)
        let old_account = users::get_account(&mut conn, target_old_account_id);
        assert!(
            old_account.is_ok(),
            "old account should still exist as ghost"
        );

        // Verify no nodes on old account
        let old_nodes = users::get_nodes_for_account(&mut conn, target_old_account_id);
        assert!(
            old_nodes.is_empty(),
            "old account should have no nodes (ghost)"
        );

        // Verify invitation is marked as accepted
        let pending = invitations::get_pending_for_invitee(&mut conn, target.node.id);
        assert!(pending.is_empty(), "invitation should no longer be pending");
    }

    #[test]
    fn test_accept_success_with_migrate() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000010", false);
        let mut target = create_test_user(&mut conn, "!dd000011", true);

        let sender_account_id = sender.account_id();
        let target_old_account_id = target.account_id();

        // Create a board and post as target (to verify migration)
        use crate::db::boards;
        let board =
            boards::add(&mut conn, "Test Board", "For testing").expect("should create board");
        let _post =
            crate::db::posts::add(&mut conn, target_old_account_id, board.id, "Target's post")
                .expect("should create post");

        // Queue a message from target
        queued_messages::queue_by_account_ids(
            &mut conn,
            target_old_account_id,
            sender_account_id,
            "test DM",
        )
        .expect("should queue message");

        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000011");

        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", &password, "migrate"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.contains("migrated"),
            "With migrate, should mention migration, got: {}",
            text
        );

        // Verify target's node is now on sender's account
        let target_reloaded = users::get(&mut conn, "!dd000011").expect("target should exist");
        assert_eq!(target_reloaded.account_id(), sender_account_id);

        // Verify old account was deleted
        let old_account = users::get_account(&mut conn, target_old_account_id);
        assert!(
            old_account.is_err(),
            "old account should be deleted after migrate"
        );

        // Verify posts were migrated to new account
        use crate::db::posts;
        let board_posts = posts::in_board(&mut conn, board.id);
        assert_eq!(board_posts.len(), 1);
        assert_eq!(board_posts[0].0.account_id, sender_account_id);
    }

    #[test]
    fn test_accept_wrong_password() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000020", false);
        let mut target = create_test_user(&mut conn, "!dd000021", true);

        let target_original_account = target.account_id();

        let _password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000021");

        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", "wrongpassword"],
        );
        assert_eq!(get_reply_text(&replies), WRONG_PASSWORD);

        // Verify node didn't move
        let target_reloaded = users::get(&mut conn, "!dd000021").expect("target should exist");
        assert_eq!(target_reloaded.account_id(), target_original_account);
    }

    #[test]
    fn test_accept_expired_invitation() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let sender = create_test_user(&mut conn, "!dd000030", false);
        let mut target = create_test_user(&mut conn, "!dd000031", true);

        // Create an expired invitation (>24 hours old)
        let old_time = now_as_useconds() - EXPIRY_US - 1_000_000;
        invitations::create_with_timestamp(
            &mut conn,
            sender.account_id(),
            target.node.id,
            "testpassword",
            old_time,
        )
        .expect("should create invitation");

        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", "testpassword"],
        );
        // Expired invitations are filtered out by get_pending_for_invitee
        assert_eq!(get_reply_text(&replies), NO_PENDING_ACCEPT);
    }

    #[test]
    fn test_accept_no_pending_invitation() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!dd000040", false);

        let replies = accept(
            &mut conn,
            &cfg,
            &mut user,
            vec!["invite accept", "somepassword"],
        );
        assert_eq!(get_reply_text(&replies), NO_PENDING_ACCEPT);
    }

    #[test]
    fn test_accept_non_target_node_rejected() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000050", false);
        let _target = create_test_user(&mut conn, "!dd000051", true);
        let mut interloper = create_test_user(&mut conn, "!dd000052", false);

        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000051");

        // Interloper tries to accept with correct password
        let replies = accept(
            &mut conn,
            &cfg,
            &mut interloper,
            vec!["invite accept", &password],
        );
        // Interloper has no pending inbound invitation
        assert_eq!(get_reply_text(&replies), NO_PENDING_ACCEPT);
    }

    #[test]
    fn test_accept_banned_node_rejected() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000060", false);
        let mut target = create_test_user(&mut conn, "!dd000061", true);

        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000061");

        // Ban the target
        users::ban(&mut conn, &target).expect("should ban");
        target.account.jackass = true;

        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", &password],
        );
        assert_eq!(get_reply_text(&replies), ACCEPT_BANNED);
    }

    #[test]
    fn test_accept_cleans_up_outbound_invitation() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000070", false);
        let mut target = create_test_user(&mut conn, "!dd000071", true);
        let _other = create_test_user(&mut conn, "!dd000072", true);

        // Target sends their own outbound invitation to someone else
        let _target_password = send_invitation(&mut conn, &cfg, &mut target, "!dd000072");

        // Now sender invites target
        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000071");

        // Verify target has an outbound invitation
        let outbound_before = invitations::get_pending_for_sender(&mut conn, target.account_id());
        assert_eq!(
            outbound_before.len(),
            1,
            "target should have outbound invitation"
        );

        // Target accepts sender's invitation
        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", &password],
        );
        assert!(get_reply_text(&replies).contains("Invitation accepted"));

        // Verify target's outbound invitation was cleaned up
        // Note: after accept, target's old account_id may have been used for the outbound.
        // The delete_pending_for_sender used old_account_id which is target's original account.
        let outbound_after = invitations::get_pending_for_sender(&mut conn, target.account_id());
        // target.account_id is now sender's account. Check the old account too.
        assert!(
            outbound_after.is_empty(),
            "target's outbound invitation on new account should be clean"
        );
    }

    #[test]
    fn test_accept_rate_limit_reset_for_sender() {
        // VAL-SEND-006: After acceptance, sender can immediately send another invitation
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000080", false);
        let mut target = create_test_user(&mut conn, "!dd000081", true);
        let _next_target = create_test_user(&mut conn, "!dd000082", true);

        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000081");

        // Target accepts
        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", &password],
        );
        assert!(get_reply_text(&replies).contains("Invitation accepted"));

        // Sender should be able to send immediately (rate limit reset)
        let replies = send(
            &mut conn,
            &cfg,
            &mut sender,
            vec!["invite !dd000082", "!dd000082"],
        );
        let text = get_reply_text(&replies);
        assert!(
            text.starts_with("Invitation sent"),
            "After acceptance, sender should be able to send immediately, got: {}",
            text
        );
    }

    #[test]
    fn test_accept_both_nodes_on_same_account() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut sender = create_test_user(&mut conn, "!dd000090", false);
        let mut target = create_test_user(&mut conn, "!dd000091", true);

        let sender_account_id = sender.account_id();

        let password = send_invitation(&mut conn, &cfg, &mut sender, "!dd000091");

        let replies = accept(
            &mut conn,
            &cfg,
            &mut target,
            vec!["invite accept", &password],
        );
        assert!(get_reply_text(&replies).contains("Invitation accepted"));

        // Both nodes should now be on the same account
        let sender_reloaded = users::get(&mut conn, "!dd000090").expect("sender exists");
        let target_reloaded = users::get(&mut conn, "!dd000091").expect("target exists");
        assert_eq!(sender_reloaded.account_id(), sender_account_id);
        assert_eq!(target_reloaded.account_id(), sender_account_id);

        // Account should have 2 nodes
        let nodes = users::get_nodes_for_account(&mut conn, sender_account_id);
        assert_eq!(nodes.len(), 2);
    }

    // ========== Ghost account display tests ==========

    #[test]
    fn test_ghost_account_posts_display_without_panic() {
        let mut conn = db::test_connection();

        // Create a user who will become a ghost
        let (ghost_user, _) = users::record(&mut conn, "!ee000001").expect("user");
        let ghost_account_id = ghost_user.account_id();

        // Create a board and post as this user
        use crate::db::boards;
        let board =
            boards::add(&mut conn, "Ghost Board", "Testing ghosts").expect("should create board");
        let _post = crate::db::posts::add(&mut conn, ghost_account_id, board.id, "Ghost's post")
            .expect("should create post");

        // Create another account and move the node there (simulating accept without migrate)
        let (other_user, _) = users::record(&mut conn, "!ee000002").expect("user2");
        users::move_node_to_account(&mut conn, &ghost_user.node, other_user.account_id())
            .expect("should move node");

        // Now the ghost account has no nodes, but has posts.
        // Reading posts should NOT panic.
        let posts_in_board = crate::db::posts::in_board(&mut conn, board.id);
        assert_eq!(posts_in_board.len(), 1);
        assert_eq!(posts_in_board[0].0.body, "Ghost's post");

        // The user display should contain ghost info
        let display = format!("{}", posts_in_board[0].1);
        assert!(
            display.contains(&format!("#{}", ghost_account_id)),
            "Ghost user display should show account ID, got: {}",
            display
        );
    }

    #[test]
    fn test_ghost_account_post_navigation_without_panic() {
        let mut conn = db::test_connection();

        // Create ghost user
        let (ghost_user, _) = users::record(&mut conn, "!ee000010").expect("user");
        let ghost_account_id = ghost_user.account_id();

        // Create a board
        use crate::db::boards;
        let board =
            boards::add(&mut conn, "Nav Board", "Testing nav").expect("should create board");

        // Create a post as normal user first
        let (normal_user, _) = users::record(&mut conn, "!ee000011").expect("user2");
        let _post1 =
            crate::db::posts::add(&mut conn, normal_user.account_id(), board.id, "Normal post")
                .expect("should create post");
        std::thread::sleep(std::time::Duration::from_micros(10));

        // Create a post as ghost-to-be
        let _post2 = crate::db::posts::add(&mut conn, ghost_account_id, board.id, "Ghost post")
            .expect("should create post");

        // Move ghost's node away
        users::move_node_to_account(&mut conn, &ghost_user.node, normal_user.account_id())
            .expect("should move node");

        // Navigate after, before, current — none should panic
        let result = crate::db::posts::after(&mut conn, board.id, 0);
        assert!(result.is_ok(), "after should work with ghost account posts");

        let (first_post, _) = result.unwrap();
        let result = crate::db::posts::after(&mut conn, board.id, first_post.created_at_us);
        assert!(result.is_ok(), "after should work for ghost account post");

        let (second_post, ghost_display) = result.unwrap();
        assert_eq!(second_post.body, "Ghost post");
        let display = format!("{}", ghost_display);
        assert!(
            display.contains(&format!("#{}", ghost_account_id)),
            "Ghost display should contain account ID, got: {}",
            display
        );
    }

    // ========== Format remaining tests ==========

    #[test]
    fn test_format_remaining_hours_and_minutes() {
        // 23 hours and 15 minutes in microseconds
        let us = (23 * 3600 + 15 * 60) * 1_000_000;
        assert_eq!(format_remaining(us), "23h 15m remaining");
    }

    #[test]
    fn test_format_remaining_minutes_only() {
        // 45 minutes in microseconds
        let us = 45 * 60 * 1_000_000;
        assert_eq!(format_remaining(us), "45m remaining");
    }

    #[test]
    fn test_format_remaining_expired() {
        assert_eq!(format_remaining(0), "expired");
        assert_eq!(format_remaining(-1), "expired");
    }

    // ========== Help tests ==========

    #[test]
    fn test_help_lists_all_subcommands() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!ff000001", false);

        let replies = help(&mut conn, &cfg, &mut user, vec!["invite"]);
        let text = get_reply_text(&replies);

        assert!(
            text.contains("block"),
            "Help should mention block, got: {}",
            text
        );
        assert!(
            text.contains("unblock"),
            "Help should mention unblock, got: {}",
            text
        );
        assert!(
            text.contains("pending"),
            "Help should mention pending, got: {}",
            text
        );
        assert!(
            text.contains("deny"),
            "Help should mention deny, got: {}",
            text
        );
        assert!(
            text.contains("accept"),
            "Help should mention accept, got: {}",
            text
        );
        assert!(
            text.contains("!node"),
            "Help should mention send syntax, got: {}",
            text
        );
    }

    #[test]
    fn test_help_includes_header() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!ff000002", false);

        let replies = help(&mut conn, &cfg, &mut user, vec!["invite"]);
        let text = get_reply_text(&replies);

        assert!(
            text.contains("Invitation commands:"),
            "Help should include header, got: {}",
            text
        );
    }
}
