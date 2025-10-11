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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, boards, users};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn fetches_skip_posts_from_jackass_users() {
        let mut conn = db::test_connection();

        let board =
            boards::add(&mut conn, "General", "General discussion").expect("should create board");

        let (user1, _) = users::record(&mut conn, "!20000001").expect("user1");
        sleep(Duration::from_micros(10));
        let (user2, _) = users::record(&mut conn, "!20000002").expect("user2");
        sleep(Duration::from_micros(10));
        let (user3, _) = users::record(&mut conn, "!20000003").expect("user3");

        let post1 = add(&mut conn, user1.id, board.id, "hello world").expect("post1");
        sleep(Duration::from_micros(10));
        let _post2 = add(&mut conn, user2.id, board.id, "buy now").expect("post2");
        sleep(Duration::from_micros(10));
        let post3 = add(&mut conn, user3.id, board.id, "all good").expect("post3");

        let _ = users::ban(&mut conn, &user2).expect("should mark jackass");

        let (next_post, next_user) = after(&mut conn, board.id, post1.created_at_us)
            .expect("should find next non-jackass post");
        assert_eq!(next_post.id, post3.id);
        assert_eq!(next_user.id, user3.id);

        let (current_post, current_user) =
            current(&mut conn, board.id, post1.created_at_us).expect("should fetch current post");
        assert_eq!(current_post.id, post1.id);
        assert_eq!(current_user.id, user1.id);

        let (previous_post, previous_user) = before(&mut conn, board.id, post3.created_at_us)
            .expect("should skip jackass when going backwards");
        assert_eq!(previous_post.id, post1.id);
        assert_eq!(previous_user.id, user1.id);

        let timeline = in_board(&mut conn, board.id);
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].0.id, post1.id);
        assert_eq!(timeline[2].0.id, post3.id);
    }
}
