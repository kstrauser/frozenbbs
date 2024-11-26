use super::models::{Board, NewBoard};
use super::schema::boards::dsl;
use super::schema::boards::table;
use super::Result;
use crate::now_as_useconds;
use diesel::prelude::*;
use validator::Validate as _;

pub fn add(conn: &mut SqliteConnection, name: &str, description: &str) -> Result<Board> {
    let new_board = NewBoard {
        name: name.trim(),
        description: description.trim(),
        created_at_us: &now_as_useconds(),
    };
    new_board.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_board)
        .returning(Board::as_returning())
        .get_result(conn)
        .expect("Error saving new board"))
}

pub fn all(conn: &mut SqliteConnection) -> Vec<Board> {
    dsl::boards
        .select(Board::as_select())
        .order(dsl::id)
        .load(conn)
        .expect("Error loading boards")
}

pub fn get(conn: &mut SqliteConnection, board_id: i32) -> QueryResult<Board> {
    dsl::boards
        .select(Board::as_select())
        .filter(dsl::id.eq(board_id))
        .first(conn)
}
