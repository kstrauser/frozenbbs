use super::Replies;
use crate::db::{boards, User};
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
    linefeed!(out);
    out.push(system_info(cfg));
    linefeed!(out);
    out.push("Send 'h' to show help options.".to_string());
    out.into()
}
