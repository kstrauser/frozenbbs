use super::models::{NewPost, Post, User};
use super::schema::posts::dsl as posts_dsl;
use super::schema::posts::table;
use super::schema::users::dsl as users_dsl;
use super::Result;
use diesel::prelude::*;
use validator::Validate as _;

pub fn add(conn: &mut SqliteConnection, user_id: i32, board_id: i32, body: &str) -> Result<Post> {
    let new_post = NewPost {
        user_id,
        board_id,
        body,
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
        .order(posts_dsl::created_at)
        .load::<(Post, User)>(conn)
        .expect("Error loading posts")
}
