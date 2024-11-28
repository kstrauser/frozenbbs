use super::models::{NewUser, User};
use super::schema::users::{dsl, table};
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

/// Record a user that we passively saw in a NodeInfo packet.
pub fn observe(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: &str,
    long_name: &str,
) -> Result<User> {
    let now = now_as_useconds();
    let new_user = NewUser {
        node_id: node_id.trim(),
        short_name: short_name.trim(),
        long_name: long_name.trim(),
        created_at_us: &now,
        last_seen_at_us: &now,
    };
    new_user.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(conn)
        .expect("Error saving new user"))
}

pub fn update(conn: &mut SqliteConnection, node_id: &str, short_name: &str, long_name: &str) {
    diesel::update(dsl::users.filter(dsl::node_id.eq(node_id)))
        .set((
            dsl::short_name.eq(short_name),
            dsl::long_name.eq(long_name),
            dsl::last_seen_at_us.eq(now_as_useconds()),
        ))
        .execute(conn)
        .expect("Error updating last seen timestamp");
}

/// Update the user's last acted and seen timestamps.
pub fn acted(conn: &mut SqliteConnection, node_id: &str) {
    let now = now_as_useconds();
    diesel::update(dsl::users.filter(dsl::node_id.eq(node_id)))
        .set((
            dsl::last_acted_at_us.eq(&now),
            dsl::last_seen_at_us.eq(&now),
        ))
        .execute(conn)
        .expect("Error updating last acted and seen timestamps");
}

pub fn all(conn: &mut SqliteConnection) -> Vec<User> {
    dsl::users
        .select(User::as_select())
        .order(dsl::created_at_us)
        .load(conn)
        .expect("Error loading users")
}

pub fn ban(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<User> {
    diesel::update(dsl::users.filter(dsl::node_id.eq(node_id)))
        .set(dsl::jackass.eq(true))
        .get_result(conn)
}
pub fn unban(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<User> {
    diesel::update(dsl::users.filter(dsl::node_id.eq(node_id)))
        .set(dsl::jackass.eq(false))
        .get_result(conn)
}

pub fn get(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<User> {
    dsl::users
        .select(User::as_select())
        .filter(dsl::node_id.eq(node_id))
        .first(conn)
}

pub fn enter_board(conn: &mut SqliteConnection, user: &User, board_id: i32) -> QueryResult<User> {
    diesel::update(dsl::users.filter(dsl::node_id.eq(&user.node_id)))
        .set(dsl::in_board.eq(board_id))
        .get_result(conn)
}
