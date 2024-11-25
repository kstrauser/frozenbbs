use crate::db::schema::{boards, posts, users};
use diesel::prelude::*;
use regex::Regex;
use time::PrimitiveDateTime;
use validator::Validate;

use once_cell::sync::Lazy;

static RE_NODE_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^![0-9a-f]{8}$").unwrap());

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::boards)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Board {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub created_at: PrimitiveDateTime,
}

#[derive(Insertable, Validate)]
#[diesel(table_name = boards)]
pub struct NewBoard<'a> {
    #[validate(length(min = 1, max = 30))]
    pub name: &'a str,

    #[validate(length(min = 1, max = 100))]
    pub description: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(belongs_to(Board))]
#[diesel(table_name = crate::db::schema::posts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Post {
    pub id: i32,
    pub board_id: i32,
    pub user_id: i32,
    pub body: String,
    pub created_at: PrimitiveDateTime,
}

#[derive(Insertable, Validate)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
    pub user_id: i32,
    pub board_id: i32,
    #[validate(length(min = 1, max = 150))]
    pub body: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub node_id: String,
    pub short_name: String,
    pub long_name: String,
    pub created_at: PrimitiveDateTime,
    pub last_seen_at: PrimitiveDateTime,
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
}
