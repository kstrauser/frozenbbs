use super::Replies;
use crate::db::{boards, users, User};
use crate::{linefeed, system_info, BBSConfig};
use diesel::SqliteConnection;

const INVALID_BOARD: &str = "That's not a valid board number.";

/// Tell the user where they are.
pub fn describe(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let mut out = vec![format!("Hi, {}!", user)];
    if let Some(user_board) = user.in_board() {
        let Ok(board) = boards::get(conn, user_board) else {
            log::error!("User {user} ended up in an unexpected board {user_board}");
            return INVALID_BOARD.into();
        };
        linefeed!(out);
        out.push(format!("You are in board {board}"));
    }

    // Show account nodes when multi-node
    let nodes = users::get_nodes_for_account(conn, user.account_id());
    if nodes.len() > 1 {
        linefeed!(out);
        out.push("Account nodes:".to_string());
        for node in &nodes {
            out.push(format!("  {} ({})", node.node_id, node.short_name));
        }
    }

    // Show invitation blocking status
    linefeed!(out);
    if user.account.invite_allowed {
        out.push("Invitations: open".to_string());
    } else {
        out.push("Invitations: blocked".to_string());
    }

    linefeed!(out);
    out.push(system_info(cfg));
    linefeed!(out);
    out.push("Send 'h' to show help options.".to_string());
    out.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::users;
    use config::Map;
    use diesel::connection::SimpleConnection;

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

    fn get_full_text(replies: &Replies) -> String {
        replies.0[0].out.join("\n")
    }

    #[test]
    fn test_describe_shows_invite_blocked_by_default() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let (mut user, _) = users::record(&mut conn, "!aabb0001").expect("should create user");

        let replies = describe(&mut conn, &cfg, &mut user, vec!["?"]);
        let text = get_full_text(&replies);
        assert!(
            text.contains("Invitations: blocked"),
            "Expected 'Invitations: blocked' in output, got: {}",
            text
        );
    }

    #[test]
    fn test_describe_shows_invite_open_when_allowed() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let (mut user, _) = users::record(&mut conn, "!aabb0002").expect("should create user");
        user = users::update_invite_allowed(&mut conn, &user, true)
            .expect("should update invite_allowed");

        let replies = describe(&mut conn, &cfg, &mut user, vec!["?"]);
        let text = get_full_text(&replies);
        assert!(
            text.contains("Invitations: open"),
            "Expected 'Invitations: open' in output, got: {}",
            text
        );
    }

    #[test]
    fn test_describe_single_node_no_node_list() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let (mut user, _) = users::record(&mut conn, "!aabb0003").expect("should create user");

        let replies = describe(&mut conn, &cfg, &mut user, vec!["?"]);
        let text = get_full_text(&replies);
        assert!(
            !text.contains("Account nodes:"),
            "Single-node account should NOT show node list, got: {}",
            text
        );
    }

    #[test]
    fn test_describe_multi_node_shows_node_list() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let (mut user, _) = users::record(&mut conn, "!aabb0004").expect("should create user");
        let account_id = user.account_id();

        // Add a second node to the same account
        let (_, _) =
            users::observe(&mut conn, "!aabb0005", Some("ND2"), Some("Node Two"), 0).unwrap();
        conn.batch_execute(&format!(
            "UPDATE nodes SET account_id = {} WHERE node_id = '!aabb0005'",
            account_id
        ))
        .expect("should reassign node");

        let replies = describe(&mut conn, &cfg, &mut user, vec!["?"]);
        let text = get_full_text(&replies);
        assert!(
            text.contains("Account nodes:"),
            "Multi-node account should show node list, got: {}",
            text
        );
        assert!(
            text.contains("!aabb0004"),
            "Node list should contain first node, got: {}",
            text
        );
        assert!(
            text.contains("!aabb0005"),
            "Node list should contain second node, got: {}",
            text
        );
        assert!(
            text.contains("ND2"),
            "Node list should show short names, got: {}",
            text
        );
    }

    #[test]
    fn test_describe_includes_greeting_and_help() {
        let mut conn = db::test_connection();
        let cfg = test_config();
        let (mut user, _) = users::record(&mut conn, "!aabb0006").expect("should create user");

        let replies = describe(&mut conn, &cfg, &mut user, vec!["?"]);
        let text = get_full_text(&replies);
        assert!(text.contains("Hi, "), "Should greet user, got: {}", text);
        assert!(
            text.contains("Send 'h' to show help options."),
            "Should include help hint, got: {}",
            text
        );
    }
}
