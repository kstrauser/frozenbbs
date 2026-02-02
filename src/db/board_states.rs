use super::models::{BoardState, NewBoardState};
use super::schema::board_states::{dsl, table};
use diesel::prelude::*;

/// Get the timestamp of the last post the account read in that board.
pub fn get(conn: &mut SqliteConnection, account_id: i32, board_id: i32) -> i64 {
    let state = table
        .select(BoardState::as_select())
        .filter(dsl::account_id.eq(account_id))
        .filter(dsl::board_id.eq(board_id))
        .first(conn)
        .optional()
        .expect("should always be possible to get board state");
    match state {
        Some(state) => state.last_post_us,
        None => 0,
    }
}

/// Store the timestamp of the post the account just read in that board.
pub fn update(conn: &mut SqliteConnection, account_id: i32, board_id: i32, last_post_us: i64) {
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // Diesel has a perfectly good ".on_conflict().do_update()" feature that works just fine.
        // The problem here is that it's most likely for the (account_id, board_id) pair to exist
        // in the database. The first time an account reads a post in a board, it gets created.
        // Every other time they read a message in that board afterward, it gets updated. And at
        // least with SQLite, the on_conflict().do_update() method increments the row ID. From its
        // docs:
        //
        // "the REPLACE algorithm deletes pre-existing rows that are causing the constraint
        //  violation prior to inserting or updating the current row and the command continues
        //  executing normally"
        //
        // Since Diesel maps that INTEGER field to an i32, that means we could only ever have 2B
        // message reads in the entire life of the BBS. OK, that's a huge number, but it's not
        // *implausibly* huge. "Who needs more than 32 bits to store a timestamp, right?"
        //
        // We could also get rid of that autoincrement primary key altogether. We don't really use
        // it. We probably never will. Past experience makes me leery of teasing the ORM deities
        // by making them work on tables without them. So fine, it's probably less work to do the
        // transactional "gimme a row. Got one? Update it! Else insert one" shuffle.
        let state = table
            .select(BoardState::as_select())
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::board_id.eq(board_id))
            .first(conn)
            .optional()
            .expect("should be able to select for an account's board state");
        if let Some(state) = state {
            diesel::update(&state)
                .set(dsl::last_post_us.eq(last_post_us))
                .execute(conn)
                .expect("should be able to update an account's board state");
        } else {
            let new_state = NewBoardState {
                account_id,
                board_id,
                last_post_us,
            };
            diesel::insert_into(table)
                .values(&new_state)
                .execute(conn)
                .expect("should be able to insert an account's new board state");
        }
        Ok(())
    })
    .expect("we must be able to commit database transactions");
}
