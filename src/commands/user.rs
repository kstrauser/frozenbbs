use super::Replies;
use crate::db::{users, User};
use crate::{linefeed, BBSConfig};
use diesel::SqliteConnection;

const NO_BIO: &str = "You haven't set a bio.";

/// Show the most recently active users.
pub fn user_active(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let mut out = Vec::new();
    out.push("Active users:".to_string());
    linefeed!(out);
    for user in users::recently_active(conn, 10) {
        out.push(format!("{}: {}", user.last_acted_at(), user));
    }
    out.into()
}

/// Show the most recently seen users.
pub fn user_seen(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let mut out = Vec::new();
    out.push("Seen users:".to_string());
    linefeed!(out);
    for user in users::recently_seen(conn, 10) {
        out.push(format!("{}: {}", user.last_seen_at(), user));
    }
    out.into()
}

/// Read the user's bio.
pub fn user_bio_read(
    _conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    if let Some(bio) = &user.bio {
        if !bio.is_empty() {
            return bio.to_string().into();
        }
    }
    NO_BIO.into()
}

/// Update the user's bio.
#[allow(clippy::needless_pass_by_value)]
pub fn user_bio_write(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let _ = users::update_bio(conn, user, args[0]);
    "Updated your bio.".into()
}
