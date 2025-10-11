use super::models::{User, UserNew, UserUpdate};
use super::schema::users::{dsl, table};
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

const UNKNOWN: &str = "????";

// Returned the trimmed inside of an option value if it's given.
fn option_trimmed(value: Option<&str>) -> Option<&str> {
    match value {
        Some(x) => Some(x.trim()),
        None => None,
    }
}

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
    short_name: Option<&str>,
    long_name: Option<&str>,
    last_seen_at_us: i64,
) -> Result<(User, bool)> {
    // Don't accept timestamps in the future.
    let now = now_as_useconds();
    let timestamp = if last_seen_at_us > 0 {
        last_seen_at_us.min(now)
    } else {
        now
    };

    let short_name = option_trimmed(short_name);
    let long_name = option_trimmed(long_name);

    // It's kinda wasteful to create both of these here and only use one of them, but it's more of
    // a pain in the neck to put this inside the transaction block and deal with the error types
    // there. So be it. It's not like anything here's particularly expensive.
    let new_user = UserNew {
        node_id: node_id.trim(),
        short_name: short_name.unwrap_or(UNKNOWN),
        long_name: long_name.unwrap_or(UNKNOWN),
        created_at_us: &timestamp,
        last_seen_at_us: &timestamp,
        last_acted_at_us: None,
    };
    new_user.validate()?;

    let update_user = UserUpdate {
        short_name,
        long_name,
        last_seen_at_us: Some(&now),
        last_acted_at_us: None,
        bio: None,
    };
    update_user.validate()?;

    Ok(conn
        .transaction::<_, diesel::result::Error, _>(|conn| {
            if let Ok(user) = get(conn, node_id) {
                // The user already existed. Update their short name, long name, and last seen
                // timestamps.
                Ok((
                    diesel::update(&user)
                        .set(&update_user)
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

    let new_user = UserNew {
        node_id: node_id.trim(),
        short_name: UNKNOWN,
        long_name: UNKNOWN,
        created_at_us: &now,
        last_seen_at_us: &now,
        last_acted_at_us: Some(&now),
    };
    new_user.validate()?;

    let update_user = UserUpdate {
        short_name: None,
        long_name: None,
        last_seen_at_us: Some(&now),
        last_acted_at_us: Some(&now),
        bio: None,
    };
    update_user.validate()?;

    Ok(conn
        .transaction::<_, diesel::result::Error, _>(|conn| {
            if let Ok(user) = get(conn, node_id) {
                // The user already existed. Update their last acted and last seen timestamps.
                let has_acted = user.last_acted_at_us.is_some();
                Ok((
                    diesel::update(&user)
                        .set(&update_user)
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

pub fn ban(conn: &mut SqliteConnection, user: &User) -> QueryResult<User> {
    diesel::update(&user)
        .set(dsl::jackass.eq(true))
        .get_result(conn)
}

pub fn unban(conn: &mut SqliteConnection, user: &User) -> QueryResult<User> {
    diesel::update(&user)
        .set(dsl::jackass.eq(false))
        .get_result(conn)
}

pub fn get(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<User> {
    dsl::users
        .select(User::as_select())
        .filter(dsl::node_id.eq(node_id))
        .first(conn)
}

/// Get a user by their id field.
pub fn get_by_user_id(conn: &mut SqliteConnection, user_id: i32) -> QueryResult<User> {
    dsl::users
        .select(User::as_select())
        .filter(dsl::id.eq(user_id))
        .first(conn)
}

/// Get a user by their short_name field.
///
/// short_names aren't unique. This returns Some(User) if exactly one user has the given
/// short name, or else None.
pub fn get_by_short_name(conn: &mut SqliteConnection, short_name: &str) -> Option<User> {
    let mut users = dsl::users
        .select(User::as_select())
        .filter(dsl::short_name.eq(short_name))
        .load(conn)
        .expect("should always be able to select users");
    if users.len() == 1 {
        users.pop()
    } else {
        None
    }
}

pub fn enter_board(conn: &mut SqliteConnection, user: &User, board_id: i32) -> QueryResult<User> {
    diesel::update(&user)
        .set(dsl::in_board.eq(board_id))
        .get_result(conn)
}

pub fn recently_seen(
    conn: &mut SqliteConnection,
    count: i64,
    exclude_node_id: Option<&str>,
) -> Vec<User> {
    let mut query = dsl::users
        .select(User::as_select())
        .order(dsl::last_seen_at_us.desc())
        .into_boxed();

    if let Some(node_id) = exclude_node_id {
        query = query.filter(dsl::node_id.ne(node_id));
    }

    query.limit(count).load(conn).expect("Error loading users")
}

pub fn recently_active(
    conn: &mut SqliteConnection,
    count: i64,
    exclude_node_id: Option<&str>,
) -> Vec<User> {
    let mut query = dsl::users
        .select(User::as_select())
        .filter(dsl::last_acted_at_us.is_not_null())
        .order(dsl::last_acted_at_us.desc())
        .into_boxed();

    if let Some(node_id) = exclude_node_id {
        query = query.filter(dsl::node_id.ne(node_id));
    }

    query.limit(count).load(conn).expect("Error loading users")
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

/// Update the user's biography
pub fn update_bio(conn: &mut SqliteConnection, user: &User, bio: &str) -> Result<User> {
    let update_user = UserUpdate {
        short_name: None,
        long_name: None,
        last_seen_at_us: None,
        last_acted_at_us: None,
        bio: Some(bio.to_string()),
    };
    update_user.validate()?;

    Ok(diesel::update(&user)
        .set(dsl::bio.eq(bio))
        .returning(User::as_returning())
        .get_result(conn)
        .expect("we must be able to update users"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn record_creates_and_updates_user() {
        let mut conn = db::test_connection();

        let (user, existed) = record(&mut conn, "!00000001").expect("record should succeed");
        assert!(!existed);
        assert_eq!(user.short_name, "????");
        let first_seen = user.last_seen_at_us;
        let first_acted = user.last_acted_at_us.expect("user should have acted");

        let (updated, existed_again) =
            record(&mut conn, "!00000001").expect("record should update user");
        assert!(existed_again);
        assert!(updated.last_seen_at_us >= first_seen);
        assert!(
            updated
                .last_acted_at_us
                .expect("user should still have acted")
                >= first_acted
        );
    }

    #[test]
    fn recently_active_excludes_specified_node() {
        let mut conn = db::test_connection();

        let (_, _) = record(&mut conn, "!00000001").expect("first user");
        sleep(Duration::from_micros(10));
        let (_, _) = record(&mut conn, "!00000002").expect("second user");
        sleep(Duration::from_micros(10));
        let (_, _) = record(&mut conn, "!00000003").expect("third user");

        let active = recently_active(&mut conn, 10, Some("!00000002"));
        let ids: Vec<String> = active.into_iter().map(|u| u.node_id).collect();
        assert_eq!(ids, vec!["!00000003".to_string(), "!00000001".to_string()]);
    }

    #[test]
    fn recently_seen_orders_newest_first_and_excludes_requested_node() {
        let mut conn = db::test_connection();

        let (first, _) = record(&mut conn, "!00000011").expect("first user");
        sleep(Duration::from_micros(10));
        let (second, _) = record(&mut conn, "!00000012").expect("second user");
        sleep(Duration::from_micros(10));
        let (third, _) = record(&mut conn, "!00000013").expect("third user");

        let seen = recently_seen(&mut conn, 10, Some(first.node_id.as_str()));
        let ids: Vec<String> = seen.into_iter().map(|u| u.node_id).collect();
        assert_eq!(ids, vec![third.node_id, second.node_id]);
    }
}
