use super::models::{NewPost, Post, User};
use super::schema::posts::{dsl as posts_dsl, table};
use super::schema::users::dsl as users_dsl;
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

pub fn add(conn: &mut SqliteConnection, user_id: i32, board_id: i32, body: &str) -> Result<Post> {
    let new_post = NewPost {
        user_id,
        board_id,
        body,
        created_at_us: &now_as_useconds(),
    };
    new_post.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_post)
        .returning(Post::as_returning())
        .get_result(conn)
        .expect("Error saving new post"))
}

pub fn in_board(conn: &mut SqliteConnection, board_id: i32) -> Vec<(Post, User)> {
    posts_dsl::posts
        .inner_join(users_dsl::users)
        .select((Post::as_select(), User::as_select()))
        .filter(posts_dsl::board_id.eq(board_id))
        .order(posts_dsl::created_at_us)
        .load::<(Post, User)>(conn)
        .expect("Error loading posts")
}

/// Get the post with this timestamp.
pub fn current(
    conn: &mut SqliteConnection,
    board_id: i32,
    last_timestamp: i64,
) -> QueryResult<(Post, User)> {
    posts_dsl::posts
        .inner_join(users_dsl::users)
        .select((Post::as_select(), User::as_select()))
        .filter(posts_dsl::board_id.eq(board_id))
        .filter(posts_dsl::created_at_us.eq(last_timestamp))
        .filter(users_dsl::jackass.eq(false))
        .limit(1)
        .first::<(Post, User)>(conn)
}

/// Get the first post in the board newer than this one.
pub fn after(
    conn: &mut SqliteConnection,
    board_id: i32,
    last_timestamp: i64,
) -> QueryResult<(Post, User)> {
    posts_dsl::posts
        .inner_join(users_dsl::users)
        .select((Post::as_select(), User::as_select()))
        .filter(posts_dsl::board_id.eq(board_id))
        .filter(posts_dsl::created_at_us.gt(last_timestamp))
        .filter(users_dsl::jackass.eq(false))
        .order(posts_dsl::created_at_us)
        .limit(1)
        .first::<(Post, User)>(conn)
}

/// Get the first post in the board older than this one.
pub fn before(
    conn: &mut SqliteConnection,
    board_id: i32,
    last_timestamp: i64,
) -> QueryResult<(Post, User)> {
    posts_dsl::posts
        .inner_join(users_dsl::users)
        .select((Post::as_select(), User::as_select()))
        .filter(posts_dsl::board_id.eq(board_id))
        .filter(posts_dsl::created_at_us.lt(last_timestamp))
        .filter(users_dsl::jackass.eq(false))
        .order(posts_dsl::created_at_us.desc())
        .limit(1)
        .first::<(Post, User)>(conn)
}

/// Get the number of posts
#[allow(clippy::cast_possible_truncation)] // We'll never have more than 4 billion posts.
pub fn count(conn: &mut SqliteConnection) -> i32 {
    posts_dsl::posts
        .count()
        .get_result::<i64>(conn)
        .expect("Error counting posts") as i32
}
