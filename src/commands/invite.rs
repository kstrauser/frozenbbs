use super::Replies;
use crate::db::{invitations, queued_messages, users, User};
use crate::{canonical_node_id, BBSConfig};
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
}
