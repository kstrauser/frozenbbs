use crate::db::{boards, posts, users};
use crate::formatted_tstamp;
use diesel::SqliteConnection;

pub fn user_list(connection: &mut SqliteConnection) {
    println!(
        "\
# BBS users

| Created at          | Last seen at        | Node ID   | Name | Long name                                |
| ------------------- | ------------------- | --------- | ---- | ---------------------------------------- |"
    );
    for user in users::all(connection) {
        println!(
            "| {} | {} | {} | {:4} | {:40} |",
            formatted_tstamp(user.created_at),
            formatted_tstamp(user.last_seen_at),
            user.node_id,
            user.short_name,
            user.long_name,
        );
    }
}

pub fn user_add(
    connection: &mut SqliteConnection,
    node_id: &str,
    short_name: &str,
    long_name: &str,
) {
    let user = users::add(connection, node_id, short_name, long_name).unwrap();
    println!("Created user #{}, '{}'", user.id, user.node_id);
}

pub fn board_list(connection: &mut SqliteConnection) {
    println!(
        "\
# BBS boards

| Created at          | Num | Name                           | Description |
| ------------------- | --- | ------------------------------ | ----------- |"
    );
    for board in boards::all(connection) {
        println!(
            "| {} | {:3} | {:30} | {} |",
            formatted_tstamp(board.created_at),
            board.id,
            board.name,
            board.description,
        );
    }
}

pub fn board_add(connection: &mut SqliteConnection, name: &str, description: &str) {
    let board = boards::add(connection, name, description).unwrap();
    println!("Created board #{}, '{}'", board.id, board.name);
}

pub fn post_read(connection: &mut SqliteConnection, board_id: i32) {
    let board = boards::get(connection, board_id).unwrap();
    println!("# Posts in '{}'\n", board.name);

    let post_info = posts::in_board(connection, board_id);
    if post_info.is_empty() {
        println!("There are no posts in board #{}", board_id);
        return;
    }

    println!(
        "\
| Created at          | Node ID   | Body |
| ------------------- | --------- | ---- |"
    );

    for (post, user) in post_info {
        println!(
            "| {} | {} | {} |", // "| {} | {:3} | {:30} | {} |",
            formatted_tstamp(post.created_at),
            user.node_id,
            post.body,
        );
    }
}

pub fn post_add(connection: &mut SqliteConnection, board_id: i32, node_id: &str, content: &str) {
    let user = users::get(connection, node_id).unwrap();
    let post = posts::add(connection, user.id, board_id, content).unwrap();
    println!("Created post #{}", post.id);
}
