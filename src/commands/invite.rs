use super::Replies;
use crate::db::{users, User};
use crate::BBSConfig;
use diesel::SqliteConnection;

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
    fn create_test_user(
        conn: &mut SqliteConnection,
        node_id: &str,
        invite_allowed: bool,
    ) -> User {
        let (mut user, _) = users::record(conn, node_id).expect("should create user");
        if invite_allowed {
            user = users::update_invite_allowed(conn, &user, true)
                .expect("should update invite_allowed");
        }
        user
    }

    fn get_reply_text(replies: &Replies) -> String {
        replies.0[0].out.join("\n")
    }

    #[test]
    fn test_block_sets_invite_allowed_false() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0001", true);
        assert!(user.account.invite_allowed);

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user.account.invite_allowed);

        // Verify DB is actually updated
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

        // Verify DB is actually updated
        let reloaded = users::get(&mut conn, "!aabb0002").expect("should find user");
        assert!(reloaded.account.invite_allowed);
    }

    #[test]
    fn test_block_idempotent() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        // Start already blocked (default)
        let mut user = create_test_user(&mut conn, "!aabb0003", false);
        assert!(!user.account.invite_allowed);

        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user.account.invite_allowed);

        // Block again -- still returns confirmation
        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user.account.invite_allowed);
    }

    #[test]
    fn test_unblock_idempotent() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        // Start already unblocked
        let mut user = create_test_user(&mut conn, "!aabb0004", true);
        assert!(user.account.invite_allowed);

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);

        // Unblock again -- still returns confirmation
        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);
    }

    #[test]
    fn test_block_unblock_toggle() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let mut user = create_test_user(&mut conn, "!aabb0005", false);

        // Unblock
        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);

        // Block
        let replies = block(&mut conn, &cfg, &mut user, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user.account.invite_allowed);

        // Unblock again
        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now allowed.");
        assert!(user.account.invite_allowed);
    }

    #[test]
    fn test_block_is_account_wide_multi_node() {
        let mut conn = db::test_connection();
        let cfg = test_config();

        // Create account via first node and unblock invitations
        let mut user_a = create_test_user(&mut conn, "!aabb0006", true);
        let account_id = user_a.account_id();
        assert!(user_a.account.invite_allowed);

        // Block from node A
        let replies = block(&mut conn, &cfg, &mut user_a, vec!["invite block"]);
        assert_eq!(get_reply_text(&replies), "Invitations are now blocked.");
        assert!(!user_a.account.invite_allowed);

        // Verify the account-level flag is blocked in the DB
        // (since invite_allowed is on accounts table, all nodes share this flag)
        let reloaded = users::get_account(&mut conn, account_id).expect("should find account");
        assert!(!reloaded.invite_allowed);

        // Unblock and verify it's reflected at account level
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
        assert!(matches!(
            replies.0[0].destination,
            ReplyDestination::Sender
        ));

        let replies = unblock(&mut conn, &cfg, &mut user, vec!["invite unblock"]);
        assert_eq!(replies.0.len(), 1);
        assert!(matches!(
            replies.0[0].destination,
            ReplyDestination::Sender
        ));
    }
}
