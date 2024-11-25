use super::models::{NewUser, User};
use super::schema::users::dsl;
use super::schema::users::table;
use super::Result;
use diesel::prelude::*;
use validator::Validate as _;

pub fn add(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: &str,
    long_name: &str,
) -> Result<User> {
    let new_user = NewUser {
        node_id: node_id.trim(),
        short_name: short_name.trim(),
        long_name: long_name.trim(),
    };
    new_user.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(conn)
        .expect("Error saving new user"))
}

pub fn all(conn: &mut SqliteConnection) -> Vec<User> {
    dsl::users
        .select(User::as_select())
        .order(dsl::created_at)
        .load(conn)
        .expect("Error loading users")
}

pub fn get(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<User> {
    dsl::users
        .select(User::as_select())
        .filter(dsl::node_id.eq(node_id))
        .first(conn)
}
