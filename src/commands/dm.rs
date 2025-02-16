use super::{Replies, ERROR_POSTING};
use crate::db::{queued_messages, users, User};
use crate::{canonical_node_id, BBSConfig};
use diesel::SqliteConnection;

const INVALID_NODEID: &str = "The given address is invalid.";
const NO_SUCH_USER: &str = "That user does not exist.";

/// Message another user
#[allow(clippy::needless_pass_by_value)]
pub fn send(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(node_id) = args.first() else {
        return "Unable to find the recipient".into();
    };
    let Some(body) = args.get(1) else {
        return "Unable to find the message".into();
    };

    let recipient: User = if node_id.len() > 5 || node_id.starts_with('!') {
        let Some(node_id) = canonical_node_id(node_id) else {
            return INVALID_NODEID.into();
        };
        match users::get(conn, &node_id) {
            Ok(x) => x,
            Err(_) => return NO_SUCH_USER.into(),
        }
    } else {
        match users::get_by_short_name(conn, node_id) {
            Some(x) => x,
            None => return NO_SUCH_USER.into(),
        }
    };

    let Ok(post) = queued_messages::post(conn, user, &recipient, body) else {
        return ERROR_POSTING.into();
    };
    format!("Published at {}", post.created_at()).into()
}
