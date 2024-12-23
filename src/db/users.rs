use super::models::{NewUser, User};
use super::schema::users::{dsl, table};
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

/// Record a user that we passively saw in a `NodeInfo` packet.
///
/// Updates their `last_seen` timestamps. Returns the user object, and whether they were already in
/// the database.
///
/// I'm resisting the urge to refactor this and `record` to use the same underlying code. They're
/// just different enough that the code to handle both cases would be more complex than having 2
/// similar functions.
pub fn observe(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: &str,
    long_name: &str,
    last_seen_at_us: i64,
) -> Result<(User, bool)> {
    // Don't accept timestamps in the future.
    let now = now_as_useconds();
    let timestamp = if last_seen_at_us > 0 {
        last_seen_at_us.min(now)
    } else {
        now
    };

    let new_user = NewUser {
        node_id: node_id.trim(),
        short_name: short_name.trim(),
        long_name: long_name.trim(),
        created_at_us: &timestamp,
        last_seen_at_us: &timestamp,
        last_acted_at_us: None,
    };
    new_user.validate()?;

    Ok(conn
        .transaction::<_, diesel::result::Error, _>(|conn| {
            if let Ok(user) = get(conn, node_id) {
                // The user already existed. Update their short name, long name, and last seen
                // timestamps.
                Ok((
                    diesel::update(table.filter(dsl::id.eq(user.id)))
                        .set((
                            dsl::short_name.eq(short_name),
                            dsl::long_name.eq(long_name),
                            dsl::last_seen_at_us.eq(timestamp),
                        ))
                        .returning(User::as_returning())
                        .get_result(conn)
                        .expect("Error observing existing user"),
                    true,
                ))
            } else {
                // The user is new. Insert their information.
                Ok((
                    diesel::insert_into(table)
                        .values(&new_user)
                        .returning(User::as_returning())
                        .get_result(conn)
                        .expect("Error observing new user"),
                    false,
                ))
            }
        })
        .expect("we must be able to commit database transactions"))
}

/// Get information about the user executing a command.
///
/// Updates their `last_acted` and `last_seen` timestamps. Returns the user object, and whether
/// they had already interacted with the BBS.
pub fn record(conn: &mut SqliteConnection, node_id: &str) -> Result<(User, bool)> {
    let now = now_as_useconds();
    let new_user = NewUser {
        node_id: node_id.trim(),
        short_name: "????",
        long_name: "????",
        created_at_us: &now,
        last_seen_at_us: &now,
        last_acted_at_us: Some(&now),
    };
    new_user.validate()?;

    Ok(conn
        .transaction::<_, diesel::result::Error, _>(|conn| {
            if let Ok(user) = get(conn, node_id) {
                // The user already existed. Update their last acted and last seen timestamps.
                let has_acted = user.last_acted_at_us.is_some();
                Ok((
                    diesel::update(table.filter(dsl::id.eq(user.id)))
                        .set((dsl::last_acted_at_us.eq(now), dsl::last_seen_at_us.eq(now)))
                        .returning(User::as_returning())
                        .get_result(conn)
                        .expect("should be able to update a user"),
                    has_acted,
                ))
            } else {
                // The user is new. Insert their information.
                Ok((
                    diesel::insert_into(table)
                        .values(&new_user)
                        .returning(User::as_returning())
                        .get_result(conn)
                        .expect("should be able to add new user"),
                    false,
                ))
            }
        })
        .expect("we must be able to commit database transactions"))
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

pub fn recently_seen(conn: &mut SqliteConnection, count: i64) -> Vec<User> {
    dsl::users
        .select(User::as_select())
        .order(dsl::last_seen_at_us.desc())
        .limit(count)
        .load(conn)
        .expect("Error loading users")
}

pub fn recently_active(conn: &mut SqliteConnection, count: i64) -> Vec<User> {
    dsl::users
        .select(User::as_select())
        .filter(dsl::last_acted_at_us.is_not_null())
        .order(dsl::last_acted_at_us.desc())
        .limit(count)
        .load(conn)
        .expect("Error loading users")
}

/// Get the number of seen and active users.
pub fn counts(conn: &mut SqliteConnection) -> (i32, i32) {
    #[allow(clippy::cast_possible_truncation)] // We'll never have more than 4 billion users.
    let seen_users = dsl::users
        .count()
        .get_result::<i64>(conn)
        .expect("Error counting seen users") as i32;
    #[allow(clippy::cast_possible_truncation)] // We'll never have more than 4 billion users.
    let active_users = dsl::users
        .filter(dsl::last_acted_at_us.is_not_null())
        .count()
        .get_result::<i64>(conn)
        .expect("Error counting active users") as i32;

    (seen_users, active_users)
}
