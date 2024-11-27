use crate::db::{boards, posts, users};
use crate::formatted_useconds;
use diesel::SqliteConnection;

// Today, for now, it's OK to fail when running user commands! A human will see the results,
// including an explanatory traceback. It doesn't have to be pretty to be useful.

pub fn user_list(conn: &mut SqliteConnection) {
    println!(
        "\
# BBS users

| Created at          | Last seen at        | Node ID    | Name | Long name                                |
| ------------------- | ------------------- | ---------- | ---- | ---------------------------------------- |"
    );
    let mut jackasses = false;
    for user in users::all(conn) {
        println!(
            "| {} | {} | {}{} | {:4} | {:40} |",
            formatted_useconds(user.created_at_us),
            formatted_useconds(user.last_seen_at_us),
            user.node_id,
            if user.jackass { "*" } else { " " },
            user.short_name,
            user.long_name,
        );
        if user.jackass {
            jackasses = true;
        }
    }
    if jackasses {
        println!();
        println!("Users marked with '*' are jackasses.");
    }
}

pub fn user_add(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: &str,
    long_name: &str,
    jackass: &bool,
) {
    let user = users::add(conn, node_id, short_name, long_name, jackass).unwrap();
    println!("Created user #{}, '{}'", user.id, user.node_id);
}

pub fn user_ban(conn: &mut SqliteConnection, node_id: &str) {
    let user = users::ban(conn, node_id).unwrap();
    println!("Banned user #{}, '{}'", user.id, user.node_id);
}

pub fn user_unban(conn: &mut SqliteConnection, node_id: &str) {
    let user = users::unban(conn, node_id).unwrap();
    println!("Unbanned user #{}, '{}'", user.id, user.node_id);
}

pub fn board_list(conn: &mut SqliteConnection) {
    println!(
        "\
# BBS boards

| Created at          | Num | Name                           | Description |
| ------------------- | --- | ------------------------------ | ----------- |"
    );
    for board in boards::all(conn) {
        println!(
            "| {} | {:3} | {:30} | {} |",
            formatted_useconds(board.created_at_us),
            board.id,
            board.name,
            board.description,
        );
    }
}

pub fn board_add(conn: &mut SqliteConnection, name: &str, description: &str) {
    let board = boards::add(conn, name, description).unwrap();
    println!("Created board #{}, '{}'", board.id, board.name);
}

pub fn post_read(conn: &mut SqliteConnection, board_id: i32) {
    let board = boards::get(conn, board_id).unwrap();
    println!("# Posts in '{}'\n", board.name);

    let post_info = posts::in_board(conn, board_id);
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
            formatted_useconds(post.created_at_us),
            user.node_id,
            post.body,
        );
    }
}

pub fn post_add(conn: &mut SqliteConnection, board_id: i32, node_id: &str, content: &str) {
    let user = users::get(conn, node_id).unwrap();
    let post = posts::add(conn, user.id, board_id, content).unwrap();
    println!("Created post #{}", post.id);
}
