use crate::db::{board_states, boards, posts, users, Post, User};
use crate::{system_info, BBSConfig};
use diesel::SqliteConnection;
use regex::{Regex, RegexBuilder};

const NO_BOARDS: &str = "There are no boards.";
const NO_MORE_POSTS: &str = "There are no more posts in this board.";
const NO_MORE_UNREAD: &str = "There are no more unread posts in any board.";
const NO_SUCH_POST: &str = "There is no post here.";
const NOT_IN_BOARD: &str = "You are not in a board.";
const NOT_VALID: &str = "Not a valid number!";

/// To where shall I respond?
pub enum Destination {
    Sender,
    Broadcast,
}

/// Where and what to send back to the radio.
pub struct Response {
    pub out: Vec<String>,
    pub destination: Destination,
}

/// The command returns a whole Vec of Strings.
impl From<Vec<String>> for Response {
    fn from(out: Vec<String>) -> Self {
        Response {
            out,
            destination: Destination::Sender,
        }
    }
}

/// The command returns a single &str.
impl From<&str> for Response {
    fn from(out: &str) -> Self {
        Response {
            out: vec![out.to_string()],
            destination: Destination::Sender,
        }
    }
}

/// The command returns a single String.
impl From<String> for Response {
    fn from(out: String) -> Self {
        Response {
            out: vec![out],
            destination: Destination::Sender,
        }
    }
}

// The commands

/// List all the boards.
fn board_lister(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let all_boards = boards::all(conn);
    if all_boards.is_empty() {
        return Response {
            out: vec![NO_BOARDS.to_string()],
            destination: Destination::Sender,
        };
    }
    let mut out = Vec::new();
    out.push("Boards:\n".to_string());
    for board in boards::all(conn) {
        out.push(format!(
            "#{} {}: {}",
            board.id, board.name, board.description
        ));
    }
    out.into()
}

/// Enter a board.
fn board_enter(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Response {
    let num = match args[0].parse::<i32>() {
        Ok(num) => num,
        Err(_) => {
            return NOT_VALID.into();
        }
    };
    let count = boards::count(conn);
    if count == 0 {
        return NO_BOARDS.into();
    }
    if num < 1 || num > count {
        return format!("Board number must be between 1 and {}", count).into();
    }
    let _ = users::enter_board(conn, user, num);
    format!("Entering board {}", num).into()
}

/// Print a post and information about its author.
fn post_print(post: &Post, user: &User) -> Vec<String> {
    vec![
        format!("From: {}", user),
        format!("At: {}", post.created_at()),
        post.body.to_string(),
    ]
}

/// Get the current message in the board.
fn board_current(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.into();
        }
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::current(conn, in_board, last_seen) {
        post_print(&post, &post_user).into()
    } else {
        NO_SUCH_POST.into()
    }
}

/// Get the previous message in the board.
fn board_previous(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.into();
        }
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::before(conn, in_board, last_seen) {
        board_states::update(conn, user.id, in_board, post.created_at_us);
        post_print(&post, &post_user).into()
    } else {
        NO_MORE_POSTS.into()
    }
}

/// Get the next message in the board.
fn board_next(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.into();
        }
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::after(conn, in_board, last_seen) {
        board_states::update(conn, user.id, in_board, post.created_at_us);
        post_print(&post, &post_user).into()
    } else {
        NO_MORE_POSTS.into()
    }
}

///Get the next unread message in any board.
fn board_quick(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let in_board = user.in_board.unwrap_or(1);
    let mut boards: Vec<i32> = Vec::new();
    boards.extend(in_board..=boards::count(conn));
    boards.extend(1..in_board);

    for board in boards {
        let last_seen = board_states::get(conn, user.id, board);
        if let Ok((post, post_user)) = posts::after(conn, board, last_seen) {
            board_states::update(conn, user.id, board, post.created_at_us);
            return post_print(&post, &post_user).into();
        }
    }

    NO_MORE_UNREAD.into()
}

/// Add a new post to the board.
fn board_write(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Response {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.into();
        }
    };
    let post = posts::add(conn, user.id, in_board, args[0]).unwrap();
    format!("Published at {}", post.created_at()).into()
}

/// Tell the user where they are.
pub fn state_describe(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return format!("You are {}.", user).into();
        }
    };
    let board = boards::get(conn, in_board).unwrap();
    vec![
        format!("You are {} in board #{}: {}.\n", user, in_board, board.name),
        system_info(cfg),
    ]
    .into()
}

/// Show the most recently active users.
pub fn user_active(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let mut out = Vec::new();
    out.push("Active users:\n".to_string());
    for user in users::recently_active(conn, 10) {
        out.push(format!("{}: {}", user.last_acted_at(), user));
    }
    out.into()
}

/// Show the most recently seen users.
pub fn user_seen(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Response {
    let mut out = Vec::new();
    out.push("Seen users:\n".to_string());
    for user in users::recently_seen(conn, 10) {
        out.push(format!("{}: {}", user.last_seen_at(), user));
    }
    out.into()
}

/// Show the user all commands available to them right now.
pub fn help(cfg: &BBSConfig, user: &User, commands: &Vec<Command>) -> Vec<String> {
    let mut out = Vec::new();
    out.push("Commands:\n".to_string());
    // Get the width of the widest argument of any available command.
    for command in commands {
        if (command.available)(user, cfg) {
            out.push(format!("{} : {}", command.arg, command.help));
        }
    }
    out.push("H : This help".to_string());
    out
}

// Sysop commands

/// Send a BBS advertisement to the main channel.
pub fn sysop_advertise(
    _conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Response {
    Response {
        out: vec![
            "Hi! I'm running a new BBS program here that I'm developing.".to_string(),
            "".to_string(),
            system_info(cfg),
            "".to_string(),
            "DM me to try it out!".to_string(),
        ],
        destination: Destination::Broadcast,
    }
}

// Contexts in which certain actions may be available

/// These commands are always available.
fn available_always(_user: &User, _cfg: &BBSConfig) -> bool {
    true
}

/// These commands are available to sysops.
fn available_to_sysops(user: &User, cfg: &BBSConfig) -> bool {
    cfg.sysops.contains(&user.node_id)
}

/// Return whether the user is in a message board.
fn available_in_board(user: &User, _cfg: &BBSConfig) -> bool {
    user.in_board.is_some()
}

// Build the collection of defined commands

/// Information about a command a user can execute.
pub struct Command {
    /// Help text showing the user what to send.
    arg: String,
    /// What the command does.
    help: String,
    /// The pattern matching the command and its arguments.
    pub pattern: Regex,
    /// A function that determines whether the user in this state can run this command.
    pub available: fn(&User, &BBSConfig) -> bool,
    /// The function that implements this command.
    pub func: fn(&mut SqliteConnection, &BBSConfig, &mut User, Vec<&str>) -> Response,
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
            arg: "U".to_string(),
            help: "Recently active users".to_string(),
            pattern: make_pattern("u"),
            available: available_always,
            func: user_active,
        },
        Command {
            arg: "S".to_string(),
            help: "Recently seen users".to_string(),
            pattern: make_pattern("s"),
            available: available_always,
            func: user_seen,
        },
        Command {
            arg: "B".to_string(),
            help: "Board list".to_string(),
            pattern: make_pattern("b"),
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
            arg: "Q".to_string(),
            help: "Read the next unread message in any board".to_string(),
            pattern: make_pattern("q"),
            available: available_always,
            func: board_quick,
        },
        Command {
            arg: "P".to_string(),
            help: "Read the previous message".to_string(),
            pattern: make_pattern("p"),
            available: available_in_board,
            func: board_previous,
        },
        Command {
            arg: "R".to_string(),
            help: "Read the current message".to_string(),
            pattern: make_pattern("r"),
            available: available_in_board,
            func: board_current,
        },
        Command {
            arg: "N".to_string(),
            help: "Read the next message".to_string(),
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
            arg: "!A".to_string(),
            help: "Send an advertisement to channel 0.".to_string(),
            pattern: make_pattern("!a"),
            available: available_to_sysops,
            func: sysop_advertise,
        },
        Command {
            arg: "?".to_string(),
            help: "Who and where am I?".to_string(),
            pattern: make_pattern(r"\?"),
            available: available_always,
            func: state_describe,
        },
    ]
}
