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
    // args[0] is the full command line; the first captured group is at index 1.
    let input = args.get(1).copied().unwrap_or("ping");
    pong_with_case(input).into()
}

#[cfg(test)]
mod tests {
    use super::pong_with_case;

    #[test]
    fn preserves_common_case_patterns() {
        assert_eq!(pong_with_case("ping"), "pong");
        assert_eq!(pong_with_case("Ping"), "Pong");
        assert_eq!(pong_with_case("PING"), "PONG");
        assert_eq!(pong_with_case("pInG"), "pOnG");
    }
}
