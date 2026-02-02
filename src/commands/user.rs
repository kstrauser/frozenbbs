use super::Replies;
use crate::db::{users, User};
use crate::{linefeed, BBSConfig};
use diesel::SqliteConnection;

const NO_BIO: &str = "You haven't set a bio.";
const MISSING_BIO: &str = "Unable to find the bio.";
const MISSING_NAME: &str = "Please provide a username.";

/// Show the most recently active users.
pub fn active(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let mut out = Vec::new();
    out.push("Active users:".to_string());
    linefeed!(out);
    for user in users::recently_active(conn, 10, Some(&cfg.my_id)) {
        out.push(format!("{}: {}", user.last_acted_at(), user));
    }
    out.into()
}

/// Show the most recently seen users.
pub fn seen(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let mut out = Vec::new();
    out.push("Seen users:".to_string());
    linefeed!(out);
    for user in users::recently_seen(conn, 10, Some(&cfg.my_id)) {
        out.push(format!("{}: {}", user.last_seen_at(), user));
    }
    out.into()
}

/// Read the user's bio.
pub fn bio_read(
    _conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    if let Some(bio) = user.bio() {
        if !bio.is_empty() {
            return bio.to_string().into();
        }
    }
    NO_BIO.into()
}

/// Update the user's bio.
#[allow(clippy::needless_pass_by_value)]
pub fn bio_write(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(bio) = args.get(1) else {
        return MISSING_BIO.into();
    };
    let _ = users::update_bio(conn, user, bio);
    "Updated your bio.".into()
}

/// Show the user's current username.
pub fn name_read(
    _conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    format!("Your name is: {}", user.display_name()).into()
}

/// Set the user's username.
#[allow(clippy::needless_pass_by_value)]
pub fn name_write(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(name) = args.get(1) else {
        return MISSING_NAME.into();
    };
    let name = name.trim();
    if name.is_empty() {
        return MISSING_NAME.into();
    }
    let _ = users::update_username(conn, user, Some(name));
    format!("Your name is now: {}", name).into()
}

/// Clear the user's username (revert to node's long_name).
pub fn name_clear(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let _ = users::update_username(conn, user, None);
    format!("Your name is now: {}", user.long_name()).into()
}
