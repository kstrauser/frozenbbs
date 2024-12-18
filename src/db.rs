pub mod board_states;
pub mod boards;
pub mod posts;
pub mod users;
use chrono::{Local, TimeZone, Utc};
pub use models::{Board, Post, User};
mod models;
mod schema;
use crate::BBSConfig;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use validator::ValidationErrors;

pub type Result<T> = std::result::Result<T, ValidationErrors>;

pub fn establish_connection(cfg: &BBSConfig) -> SqliteConnection {
    let database_url = &cfg.db_path;
    let mut connection = SqliteConnection::establish(database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
    connection
        .batch_execute("PRAGMA foreign_keys = ON")
        .unwrap();
    connection
}

/// Get the number of microseconds since the Unix epoch.
pub fn now_as_useconds() -> i64 {
    Utc::now().timestamp_micros()
}

/// Format the number of microseconds since the Unix epoch as a local timestamp.
fn formatted_useconds(dstamp: i64) -> String {
    format!(
        "{}",
        Local
            .timestamp_micros(dstamp)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S")
    )
}
