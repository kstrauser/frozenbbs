use crate::db::{boards, posts, users};
use crate::db::{Post, User};
use diesel::SqliteConnection;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::io::{self, Write as _};

const NOT_IN_BOARD: &str = "You are not in a board.";

/// The user's global state
#[derive(Debug)]
struct UserState<'a> {
    node_id: &'a str,
    last_seen: HashMap<i32, i64>,
}

fn board_lister(
    conn: &mut SqliteConnection,
    _user: &mut User,
    _state: &mut UserState,
    _args: Vec<&str>,
) {
    println!("Boards:");
    println!();
    let all_boards = boards::all(conn);
    if all_boards.is_empty() {
        println!("There are no boards.");
    } else {
        for board in boards::all(conn) {
            println!("#{} {}: {}", board.id, board.name, board.description);
        }
    }
}

fn board_enter(
    conn: &mut SqliteConnection,
    user: &mut User,
    state: &mut UserState,
    args: Vec<&str>,
) {
    let num = match args[0].parse::<i32>() {
        Ok(num) => num,
        Err(_) => {
            println!("Not a valid number!");
            return;
        }
    };
    let count = boards::count(conn);
    if count == 0 {
        println!("There are no boards.");
        return;
    }
    if num < 1 || num > count {
        println!("Board number must be between 1 and {}", count);
        return;
    }
    println!("Entering board {}", num);
    let _ = users::enter_board(conn, user, num);
    state.last_seen.entry(num).or_insert(0);
}

/// Print a post and information about its author.
fn post_print(post: &Post, user: &User) {
    println!("From: {}", user);
    println!("At  : {}", post.created_at());
    println!("Msg : {}", post.body);
}

fn board_previous(
    conn: &mut SqliteConnection,
    user: &mut User,
    state: &mut UserState,
    _args: Vec<&str>,
) {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            println!("{}", NOT_IN_BOARD);
            return;
        }
    };
    if let Ok((post, user)) =
        posts::before(conn, in_board, *state.last_seen.get(&in_board).unwrap())
    {
        post_print(&post, &user);
        state.last_seen.insert(in_board, post.created_at_us);
    } else {
        println!("There are no more posts in this board.");
    }
}

fn board_next(
    conn: &mut SqliteConnection,
    user: &mut User,
    state: &mut UserState,
    _args: Vec<&str>,
) {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            println!("{}", NOT_IN_BOARD);
            return;
        }
    };
    if let Ok((post, user)) = posts::after(conn, in_board, *state.last_seen.get(&in_board).unwrap())
    {
        post_print(&post, &user);
        state.last_seen.insert(in_board, post.created_at_us);
    } else {
        println!("There are no more posts in this board.");
    }
}

fn board_write(
    conn: &mut SqliteConnection,
    user: &mut User,
    _state: &mut UserState,
    args: Vec<&str>,
) {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            println!("{}", NOT_IN_BOARD);
            return;
        }
    };
    let post = posts::add(conn, user.id, in_board, args[0]).unwrap();
    println!("Published at {}.", post.created_at());
}

fn state_describe(
    conn: &mut SqliteConnection,
    user: &mut User,
    _state: &mut UserState,
    _args: Vec<&str>,
) {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            println!("{}", NOT_IN_BOARD);
            return;
        }
    };
    let board = boards::get(conn, in_board).unwrap();
    println!("You are in board #{}: {}", in_board, board.name);
}

/// Return whether the user is in a message board.
fn available_in_board(user: &User, _state: &UserState) -> bool {
    user.in_board.is_some()
}

/// These commands are always available.
fn available_always(_user: &User, _state: &UserState) -> bool {
    true
}

/// Information about a command a user can execute.
struct Command {
    /// Help text showing the user what to send.
    arg: String,
    /// What the command does.
    help: String,
    /// The pattern matching the command and its arguments.
    pattern: Regex,
    /// A function that determines whether the user in this state can run this command.
    available: fn(&User, &UserState) -> bool,
    /// The function that implements this command.
    func: fn(&mut SqliteConnection, &mut User, &mut UserState, Vec<&str>),
}

fn make_pattern(pattern: &str) -> Regex {
    RegexBuilder::new(format!("^{}$", pattern).as_str())
        .case_insensitive(false)
        .build()
        .unwrap()
}

fn setup() -> Vec<Command> {
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

/// Run a session from the local terminal.
pub fn client(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: &Option<String>,
    long_name: &Option<String>,
) {
    if let Ok(user) = users::get(conn, node_id) {
        users::update(
            conn,
            node_id,
            match short_name {
                Some(x) => x,
                None => user.short_name.as_str(),
            },
            match long_name {
                Some(x) => x,
                None => user.long_name.as_str(),
            },
        );
        let user = users::get(conn, node_id).unwrap();
        println!("Welcome back, {}!", user);
    } else {
        let user = users::add(
            conn,
            node_id,
            short_name
                .as_ref()
                .expect("New users must have a short name"),
            long_name.as_ref().expect("New users must have a long name"),
            &false,
        )
        .unwrap();
        println!("Hello there, {}!", user);
    }

    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let stdin = io::stdin(); // We get `Stdin` here.
    let mut state = UserState {
        node_id,
        last_seen: HashMap::new(),
    };
    let commands = setup();

    let mut this_user = users::get(conn, node_id).unwrap();
    help(&this_user, &state, &commands);
    println!();
    state_describe(conn, &mut this_user, &mut state, [].to_vec());

    'outer: loop {
        println!();
        print!("Command: ");
        stdout.flush().unwrap();
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        println!();
        if buffer.is_empty() {
            println!("Disconnected.");
            return;
        }
        users::saw(conn, state.node_id);
        let trimmed = buffer.trim();
        let lower = trimmed.to_lowercase();

        let mut this_user = users::get(conn, node_id).unwrap();

        for command in commands.iter() {
            if !(command.available)(&this_user, &state) {
                continue;
            }
            if let Some(captures) = command.pattern.captures(trimmed) {
                (command.func)(
                    conn,
                    &mut this_user,
                    &mut state,
                    // Collect all of the matched groups in the pattern into a vector of strs
                    captures
                        .iter()
                        .skip(1)
                        .flatten()
                        .map(|x| x.as_str().trim())
                        .collect(),
                );
                continue 'outer;
            }
        }

        if lower == "q" {
            println!("buh-bye!");
            return;
        }

        println!("That's not an available command here.");
        help(&this_user, &state, &commands);
    }
}

/// Show the user all commands available to them right now.
fn help(user: &User, state: &UserState, commands: &Vec<Command>) {
    println!("\nCommands:\n");
    let width = commands
        .iter()
        .filter(|x| (x.available)(user, state))
        .map(|x| x.arg.len())
        .max()
        .unwrap();
    for command in commands {
        if (command.available)(user, state) {
            println!("{:width$} : {}", command.arg, command.help);
        }
    }
    println!("{:width$} : Quit", "Q");
}
