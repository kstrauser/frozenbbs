use clap::{Parser, Subcommand};
use diesel::prelude::*;
use frozenbbs::db::{boards, establish_connection, posts, users};
use time::{format_description::BorrowedFormatItem, macros::format_description};

const TSTAMP_FORMAT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]@[hour]:[minute]:[second]");

fn formatted_tstamp(tstamp: time::PrimitiveDateTime) -> String {
    tstamp.format(&TSTAMP_FORMAT).unwrap()
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Subsystems>,
}

#[derive(Debug, Subcommand)]
enum Subsystems {
    /// Admin commands
    #[command(arg_required_else_help = true)]
    Admin {
        #[command(subcommand)]
        admin_command: Option<AdminCommands>,
    },
}

#[derive(Debug, Subcommand)]
enum AdminCommands {
    /// User commands
    #[command(arg_required_else_help = true)]
    User {
        #[command(subcommand)]
        user_command: Option<AdminUserCommands>,
    },
    /// Board commands
    #[command(arg_required_else_help = true)]
    Board {
        #[command(subcommand)]
        board_command: Option<AdminBoardCommands>,
    },
    /// Post commands
    #[command(arg_required_else_help = true)]
    Post {
        #[command(subcommand)]
        post_command: Option<AdminPostCommands>,
    },
}

#[derive(Debug, Subcommand)]
enum AdminUserCommands {
    /// List all users.
    List {},

    /// Add a new user.
    Add {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        id: String,
        /// User's 4-byte short name.
        #[arg(short, long)]
        short: String,
        /// User's long name.
        #[arg(short, long)]
        long: String,
    },
}

#[derive(Debug, Subcommand)]
enum AdminBoardCommands {
    /// List all boards.
    List {},

    /// Add a new board.
    Add {
        /// Name of the board to add.
        #[arg(short, long)]
        name: String,
        /// Description of the new board.
        #[arg(short, long)]
        description: String,
    },
}

#[derive(Debug, Subcommand)]
enum AdminPostCommands {
    /// Read a board's posts.
    Read {
        /// Number of the board to read.
        #[arg(short, long)]
        board_id: i32,
    },

    /// Add a new post.
    Add {
        /// Number of the board to post to.
        #[arg(short, long)]
        board_id: i32,
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: String,
        /// Body of the new post.
        #[arg(short, long)]
        content: String,
    },
}

fn user_list(connection: &mut SqliteConnection) {
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

fn user_add(connection: &mut SqliteConnection, node_id: &str, short_name: &str, long_name: &str) {
    let user = users::add(connection, node_id, short_name, long_name).unwrap();
    println!("Created user #{}, '{}'", user.id, user.node_id);
}

fn board_list(connection: &mut SqliteConnection) {
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

fn board_add(connection: &mut SqliteConnection, name: &str, description: &str) {
    let board = boards::add(connection, name, description).unwrap();
    println!("Created board #{}, '{}'", board.id, board.name);
}

fn post_read(connection: &mut SqliteConnection, board_id: i32) {
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

fn post_add(connection: &mut SqliteConnection, board_id: i32, node_id: &str, content: &str) {
    let user = users::get(connection, node_id).unwrap();
    let post = posts::add(connection, user.id, board_id, content).unwrap();
    println!("Created post #{}", post.id);
}

fn main() {
    let cli = Cli::parse();

    let connection = &mut establish_connection();

    match &cli.command {
        Some(Subsystems::Admin { admin_command }) => match admin_command {
            Some(AdminCommands::User { user_command }) => match user_command {
                Some(AdminUserCommands::List {}) => user_list(connection),
                Some(AdminUserCommands::Add { id, short, long }) => {
                    user_add(connection, id, short, long)
                }
                None => {}
            },
            Some(AdminCommands::Board { board_command }) => match board_command {
                Some(AdminBoardCommands::List {}) => board_list(connection),
                Some(AdminBoardCommands::Add { name, description }) => {
                    board_add(connection, name, description)
                }
                None => {}
            },
            Some(AdminCommands::Post { post_command }) => match post_command {
                Some(AdminPostCommands::Read { board_id }) => post_read(connection, *board_id),
                Some(AdminPostCommands::Add {
                    board_id,
                    node_id,
                    content,
                }) => post_add(connection, *board_id, node_id, content),
                None => {}
            },
            None => {}
        },
        None => {}
    }
}
