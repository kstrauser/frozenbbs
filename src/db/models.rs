use super::formatted_useconds;
use super::schema::{board_states, boards, posts, users};
use diesel::prelude::*;
use regex::Regex;
use std::fmt;
use validator::Validate;

use once_cell::sync::Lazy;

static RE_NODE_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^![0-9a-f]{8}$").unwrap());
// This seems like a reasonable range to clamp timestamps to. Because we're dealing with
// microseconds, it's good to enforce a plausible range so that things will blow up if we
// inadvertently try to use seconds, milliseconds, or nanoseconds somewhere.
const EARLY_2024: i64 = 1704096000000000;
const EARLY_2200: i64 = 7258147200000000;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::boards)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Board {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub created_at_us: i64,
}

impl Board {
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = boards)]
pub struct NewBoard<'a> {
    #[validate(length(min = 1, max = 30))]
    pub name: &'a str,
    #[validate(length(min = 1, max = 100))]
    pub description: &'a str,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(belongs_to(Board))]
#[diesel(table_name = crate::db::schema::posts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Post {
    pub id: i32,
    pub board_id: i32,
    pub user_id: i32,
    pub body: String,
    pub created_at_us: i64,
}

impl Post {
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
    #[validate(range(min = 1))]
    pub user_id: i32,
    #[validate(range(min = 1))]
    pub board_id: i32,
    #[validate(length(min = 1, max = 150))]
    pub body: &'a str,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub node_id: String,
    pub short_name: String,
    pub long_name: String,
    pub jackass: bool,
    pub in_board: Option<i32>,
    pub created_at_us: i64,
    pub last_seen_at_us: i64,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}:{}", self.node_id, self.short_name, self.long_name)
    }
}

impl User {
    pub fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}:{}", self.node_id, self.short_name, self.long_name)
    }
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
    pub fn last_seen_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    #[validate(regex(path = *RE_NODE_ID))]
    pub node_id: &'a str,
    #[validate(length(min = 1, max = 4))]
    pub short_name: &'a str,
    #[validate(length(min = 1, max = 40))]
    pub long_name: &'a str,
    pub jackass: &'a bool,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_seen_at_us: &'a i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::board_states)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct BoardState {
    pub id: i32,
    // pub user_id: i32,
    // pub board_id: i32,
    pub last_post_us: i64,
}

#[derive(Debug, Insertable, Validate)]
#[diesel(table_name = board_states)]
pub struct NewBoardState {
    #[validate(range(min = 1))]
    pub user_id: i32,
    #[validate(range(min = 1))]
    pub board_id: i32,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_post_us: i64,
}
