use crate::db::{board_states, boards, posts, users, Post, User};
use diesel::SqliteConnection;
use regex::{Regex, RegexBuilder};

const NO_BOARDS: &str = "There are no boards.";
const NO_MORE_POSTS: &str = "There are no more posts in this board.";
const NOT_IN_BOARD: &str = "You are not in a board.";
const NOT_VALID: &str = "Not a valid number!";

/// List all the boards.
fn board_lister(conn: &mut SqliteConnection, _user: &mut User, _args: Vec<&str>) -> String {
    let all_boards = boards::all(conn);
    if all_boards.is_empty() {
        return format!("{}\n", NO_BOARDS);
    }
    let mut out = String::new();
    out.push_str("Boards:\n\n");
    for board in boards::all(conn) {
        out.push_str(&format!(
            "#{} {}: {}\n",
            board.id, board.name, board.description
        ));
    }
    out
}

/// Enter a board.
fn board_enter(conn: &mut SqliteConnection, user: &mut User, args: Vec<&str>) -> String {
    let num = match args[0].parse::<i32>() {
        Ok(num) => num,
        Err(_) => {
            return format!("{}\n", NOT_VALID);
        }
    };
    let count = boards::count(conn);
    if count == 0 {
        return format!("{}\n", NO_BOARDS);
    }
    if num < 1 || num > count {
        return format!("Board number must be between 1 and {}\n", count);
    }
    let _ = users::enter_board(conn, user, num);
    format!("Entering board {}\n", num)
}

/// Print a post and information about its author.
fn post_print(post: &Post, user: &User) -> String {
    format!(
        "\
From: {}
At  : {}
Msg : {}
",
        user,
        post.created_at(),
        post.body
    )
}

/// Get the previous message in the board.
fn board_previous(conn: &mut SqliteConnection, user: &mut User, _args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return format!("{}\n", NOT_IN_BOARD);
        }
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::before(conn, in_board, last_seen) {
        board_states::update(conn, user.id, in_board, post.created_at_us);
        post_print(&post, &post_user)
    } else {
        if last_seen != 0 {
            board_states::update(conn, user.id, in_board, last_seen - 1);
        }
        format!("{}\n", NO_MORE_POSTS)
    }
}

/// Get the next message in the board.
fn board_next(conn: &mut SqliteConnection, user: &mut User, _args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return format!("{}\n", NOT_IN_BOARD);
        }
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::after(conn, in_board, last_seen) {
        board_states::update(conn, user.id, in_board, post.created_at_us);
        post_print(&post, &post_user)
    } else {
        if last_seen != 0 {
            board_states::update(conn, user.id, in_board, last_seen + 1);
        }
        format!("{}\n", NO_MORE_POSTS)
    }
}

/// Add a new post to the board.
fn board_write(conn: &mut SqliteConnection, user: &mut User, args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return format!("{}\n", NOT_IN_BOARD);
        }
    };
    let post = posts::add(conn, user.id, in_board, args[0]).unwrap();
    format!("Published at {}.\n", post.created_at())
}

/// Tell the user where they are.
pub fn state_describe(conn: &mut SqliteConnection, user: &mut User, _args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return format!("{}\n", NOT_IN_BOARD);
        }
    };
    let board = boards::get(conn, in_board).unwrap();
    format!("You are in board #{}: {}\n", in_board, board.name)
}

/// Show the user all commands available to them right now.
pub fn help(user: &User, commands: &Vec<Command>) -> String {
    let mut out = String::new();
    out.push_str("Commands:\n\n");
    // Get the width of the widest argument of any available command.
    let width = commands
        .iter()
        .filter(|x| (x.available)(user))
        .map(|x| x.arg.len())
        .max()
        .unwrap();
    for command in commands {
        if (command.available)(user) {
            out.push_str(&format!("{:width$} : {}\n", command.arg, command.help));
        }
    }
    out.push_str(&format!("{:width$} : This help\n", "H"));
    out
}

/// Return whether the user is in a message board.
fn available_in_board(user: &User) -> bool {
    user.in_board.is_some()
}

/// These commands are always available.
fn available_always(_user: &User) -> bool {
    true
}

/// Information about a command a user can execute.
pub struct Command {
    /// Help text showing the user what to send.
    arg: String,
    /// What the command does.
    help: String,
    /// The pattern matching the command and its arguments.
    pub pattern: Regex,
    /// A function that determines whether the user in this state can run this command.
    pub available: fn(&User) -> bool,
    /// The function that implements this command.
    pub func: fn(&mut SqliteConnection, &mut User, Vec<&str>) -> String,
}

/// Build a Regex in our common fashion.
fn make_pattern(pattern: &str) -> Regex {
    RegexBuilder::new(format!("^{}$", pattern).as_str())
        .case_insensitive(true)
        .build()
        .unwrap()
}

pub fn setup() -> Vec<Command> {
    vec![
        Command {
            arg: "B".to_string(),
            help: "Board list".to_string(),
            pattern: make_pattern("^b$"),
            available: available_always,
            func: board_lister,
        },
        Command {
            arg: "Bn".to_string(),
            help: "Enter board #n".to_string(),
            pattern: make_pattern(r"b(\d+)"),
            available: available_always,
            func: board_enter,
        },
        Command {
            arg: "P".to_string(),
            help: "Read the previous message in the board".to_string(),
            pattern: make_pattern("p"),
            available: available_in_board,
            func: board_previous,
        },
        Command {
            arg: "N".to_string(),
            help: "Read the next message in the board".to_string(),
            pattern: make_pattern("n"),
            available: available_in_board,
            func: board_next,
        },
        Command {
            arg: "W msg".to_string(),
            help: "Write a new message".to_string(),
            pattern: make_pattern(r"w(.{1,})"),
            available: available_in_board,
            func: board_write,
        },
        Command {
            arg: "?".to_string(),
            help: "Tell me where I am".to_string(),
            pattern: make_pattern(r"\?"),
            available: available_always,
            func: state_describe,
        },
    ]
}
