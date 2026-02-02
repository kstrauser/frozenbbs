pub mod board_states;
pub mod boards;
pub mod posts;
pub mod queued_messages;
pub mod users;
use chrono::{Local, MappedLocalTime, TimeZone, Utc};
pub use models::{Account, Board, Node, Post, User};
mod models;
mod schema;
use crate::BBSConfig;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use validator::ValidationErrors;

pub type Result<T> = std::result::Result<T, ValidationErrors>;

pub fn establish_connection(cfg: &BBSConfig) -> SqliteConnection {
    let database_url = &cfg.db_path;
    let mut connection = SqliteConnection::establish(database_url).expect(
        "{database_url} should be a SQLite database file. Consider running `just migrate`.",
    );
    connection
        .batch_execute("PRAGMA foreign_keys = ON")
        .expect("should enable strict foreign key support in the database");
    connection
}

/// Get the number of microseconds since the Unix epoch.
pub fn now_as_useconds() -> i64 {
    Utc::now().timestamp_micros()
}

/// Format the number of microseconds since the Unix epoch as a local timestamp.
fn formatted_useconds(dstamp: i64) -> String {
    let fmt = "%Y-%m-%dT%H:%M:%S";
    match Local.timestamp_micros(dstamp) {
        // This should be the path except during daylight saving time changes.
        MappedLocalTime::Single(t) => t.format(fmt).to_string(),
        // I'm not 100% sure what happens right after the clock gets set back. Since the origin of
        // this timestamp is the Unix epoch in UTC, I'd think any given source timestamp would
        // map to a valid local timestamp. If it repeats values sometimes, that's only a
        // display issue for users. It's not that big of a deal. We're not running a bank here.
        MappedLocalTime::Ambiguous(t1, _) => t1.format(fmt).to_string(),
        // I don't think this should ever happen, again, because the input is the UTC Unix epoch.
        // How would we ever end up at time that doesn't exist without using a localized time
        // offset? I don't know. But deal with it anyway in case I'm missing something. Note that
        // the error message here is exactly the same length as the normal timestamp so that
        // something nitpicky about formatting will still work.
        MappedLocalTime::None => "No such local time.".to_string(),
    }
}

pub fn stats(conn: &mut SqliteConnection) -> String {
    let (seen, active) = users::counts(conn);
    format!(
        "\
Seen users  : {}
Active users: {}
Boards      : {}
Posts       : {}",
        seen,
        active,
        boards::count(conn),
        posts::count(conn)
    )
}

#[cfg(test)]
pub(crate) fn test_connection() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("should create in-memory SQLite database");
    conn.batch_execute(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE boards (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL,
            created_at_us BIGINT NOT NULL
        );
        CREATE TABLE accounts (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            username TEXT,
            jackass BOOL NOT NULL DEFAULT FALSE,
            bio TEXT,
            created_at_us BIGINT NOT NULL,
            last_acted_at_us BIGINT
        );
        CREATE TABLE nodes (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            node_id TEXT NOT NULL UNIQUE,
            short_name TEXT NOT NULL,
            long_name TEXT NOT NULL,
            in_board INTEGER,
            created_at_us BIGINT NOT NULL,
            last_seen_at_us BIGINT NOT NULL,
            FOREIGN KEY (account_id) REFERENCES accounts (id),
            FOREIGN KEY (in_board) REFERENCES boards (id)
        );
        CREATE TABLE posts (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            board_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            body TEXT NOT NULL,
            created_at_us BIGINT NOT NULL,
            UNIQUE(created_at_us),
            FOREIGN KEY (user_id) REFERENCES accounts (id),
            FOREIGN KEY (board_id) REFERENCES boards (id)
        );
        CREATE TABLE board_states (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            board_id INTEGER NOT NULL,
            last_post_us BIGINT NOT NULL,
            FOREIGN KEY (board_id) REFERENCES boards (id),
            FOREIGN KEY (user_id) REFERENCES accounts (id)
        );
        CREATE TABLE queued_messages (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            sender_id INTEGER NOT NULL,
            recipient_id INTEGER NOT NULL,
            body TEXT NOT NULL,
            created_at_us BIGINT NOT NULL,
            sent_at_us BIGINT,
            FOREIGN KEY (sender_id) REFERENCES accounts (id),
            FOREIGN KEY (recipient_id) REFERENCES accounts (id)
        );
        "#,
    )
    .expect("should create schema for tests");
    conn
}
