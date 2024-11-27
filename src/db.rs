pub mod boards;
pub mod posts;
pub mod users;
use chrono::{Local, TimeZone, Utc};
pub use models::{Board, Post, User};
mod models;
mod schema;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use dotenvy::dotenv;
use std::env;
use validator::ValidationErrors;

pub type Result<T> = std::result::Result<T, ValidationErrors>;

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut connection = SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
    connection
        .batch_execute("PRAGMA foreign_keys = ON")
        .unwrap();
    connection
}

/// Get the number of microseconds since the Unix epoch.
fn now_as_useconds() -> i64 {
    Utc::now().timestamp_micros()
}

/// Format the number of microseconds since the Unix epoch as a local timestamp.
pub fn formatted_useconds(dstamp: i64) -> String {
    format!(
        "{}",
        Local
            .timestamp_micros(dstamp)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S")
    )
}
