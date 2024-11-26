pub mod boards;
pub mod posts;
pub mod users;
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
