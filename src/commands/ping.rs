use super::Replies;
use crate::db::User;
use crate::BBSConfig;
use diesel::SqliteConnection;

/// Transform "ping" into "pong" preserving the case pattern of the input.
fn pong_with_case(input: &str) -> String {
    // Map each character of "pong" to the case of the corresponding character in the input.
    // If the input is shorter than 4 characters, remaining letters default to lowercase.
    let base = ['p', 'o', 'n', 'g'];
    let mut out = String::with_capacity(4);

    for (i, b) in base.iter().enumerate() {
        let c = if let Some(ch) = input.chars().nth(i) {
            if ch.is_uppercase() {
                b.to_ascii_uppercase()
            } else if ch.is_lowercase() {
                b.to_ascii_lowercase()
            } else {
                // Non-alphabetic: fall back to lowercase.
                b.to_ascii_lowercase()
            }
        } else {
            b.to_ascii_lowercase()
        };
        out.push(c);
    }

    out
}

/// Reply to a ping with a case-matched pong.
#[allow(clippy::needless_pass_by_value)]
pub fn ping(
    _conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    args: Vec<&str>,
) -> Replies {
    // Always use argv[0], which is the full trimmed command line text.
    let input = args.first().copied().unwrap_or("ping");
    pong_with_case(input).into()
}

#[cfg(test)]
mod tests {
    use super::{ping, pong_with_case};
    use crate::db::{Account, Node, User};
    use crate::BBSConfig;
    use config::Map;

    fn dummy_user() -> User {
        // Minimal user; fields not relevant to ping behaviour.
        User {
            account: Account {
                id: 1,
                username: None,
                jackass: false,
                bio: None,
                created_at_us: 0,
                last_acted_at_us: None,
            },
            node: Node {
                id: 1,
                account_id: 1,
                node_id: "!cafeb33d".to_string(),
                short_name: "TEST".to_string(),
                long_name: "Test User".to_string(),
                in_board: None,
                created_at_us: 0,
                last_seen_at_us: 0,
            },
        }
    }
    #[test]
    fn preserves_common_case_patterns() {
        assert_eq!(pong_with_case("ping"), "pong");
        assert_eq!(pong_with_case("Ping"), "Pong");
        assert_eq!(pong_with_case("PING"), "PONG");
        assert_eq!(pong_with_case("pInG"), "pOnG");
    }

    #[test]
    fn handles_short_or_weird_inputs() {
        assert_eq!(pong_with_case("p"), "pong");
        assert_eq!(pong_with_case("Pi"), "Pong");
        assert_eq!(pong_with_case("p!n?"), "pong");
        assert_eq!(pong_with_case(""), "pong");
    }

    #[test]
    fn ping_uses_full_command_line_and_preserves_case() {
        let mut user = dummy_user();
        let cfg = BBSConfig {
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
        };
        let mut conn = crate::db::test_connection();

        // Simulate the dispatcher passing argv[0] as the trimmed command line.
        let replies = ping(&mut conn, &cfg, &mut user, vec!["PING"]);
        assert_eq!(replies.0.len(), 1);
        assert_eq!(replies.0[0].out, vec!["PONG".to_string()]);

        let replies = ping(&mut conn, &cfg, &mut user, vec!["Ping"]);
        assert_eq!(replies.0[0].out, vec!["Pong".to_string()]);
    }
}
