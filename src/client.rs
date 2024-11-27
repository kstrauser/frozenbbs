use crate::db::{boards, posts, users};
use crate::db::{Post, User};
use crate::formatted_useconds;
use diesel::SqliteConnection;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::io::{self, Write as _};

const FAKEBOARD: i32 = -1;

/// The user's global state
#[derive(Debug)]
struct UserState<'a> {
    node_id: &'a str,
    board: i32,
    last_seen: HashMap<i32, i64>,
}

fn board_lister(conn: &mut SqliteConnection, _state: &mut UserState, _args: Vec<&str>) {
    println!("Boards:");
    println!();
    for board in boards::all(conn) {
        println!("#{} {}: {}", board.id, board.name, board.description);
    }
}

fn board_enter(conn: &mut SqliteConnection, state: &mut UserState, args: Vec<&str>) {
    let num = match args[0].parse::<i32>() {
        Ok(num) => num,
        Err(_) => {
            println!("Not a valid number!");
            return;
        }
    };
    let count = boards::count(conn);
    if num < 1 || num > count {
        println!("Board number must be between 1 and {}", count);
        return;
    }
    println!("Entering board {}", num);
    state.board = num;
    state.last_seen.entry(num).or_insert(0);
}

/// Print a post and information about its author.
fn post_print(post: &Post, user: &User) {
    println!(
        "From: {}/{}:{}",
        user.node_id, user.short_name, user.long_name
    );
    println!("At  : {}", formatted_useconds(post.created_at_us));
    println!("Msg : {}", post.body);
}

fn board_previous(conn: &mut SqliteConnection, state: &mut UserState, _args: Vec<&str>) {
    if let Ok((post, user)) = posts::before(
        conn,
        state.board,
        *state.last_seen.get(&state.board).unwrap(),
    ) {
        post_print(&post, &user);
        state.last_seen.insert(state.board, post.created_at_us);
    } else {
        println!("There are no more posts in this board.");
    }
}

fn board_next(conn: &mut SqliteConnection, state: &mut UserState, _args: Vec<&str>) {
    if let Ok((post, user)) = posts::after(
        conn,
        state.board,
        *state.last_seen.get(&state.board).unwrap(),
    ) {
        post_print(&post, &user);
        state.last_seen.insert(state.board, post.created_at_us);
    } else {
        println!("There are no more posts in this board.");
    }
}

fn board_write(conn: &mut SqliteConnection, state: &mut UserState, args: Vec<&str>) {
    let user = users::get(conn, state.node_id).unwrap();
    let post = posts::add(conn, user.id, state.board, args[0]).unwrap();
    println!("Published at {}.", formatted_useconds(post.created_at_us));
}

fn state_describe(conn: &mut SqliteConnection, state: &mut UserState, _args: Vec<&str>) {
    if state.board == FAKEBOARD {
        println!("You are not in a board.");
        return;
    }
    let board = boards::get(conn, state.board).unwrap();
    println!("You are in board #{}: {}", state.board, board.name);
}

/// Return whether the user is in a message board.
fn available_in_board(state: &UserState) -> bool {
    state.board != FAKEBOARD
}

/// These commands are always available.
fn available_always(_state: &UserState) -> bool {
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
    available: fn(&UserState) -> bool,
    /// The function that implements this command.
    func: fn(&mut SqliteConnection, &mut UserState, Vec<&str>),
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
pub fn client(conn: &mut SqliteConnection, node_id: &str) {
    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let stdin = io::stdin(); // We get `Stdin` here.
    let mut state = UserState {
        node_id,
        board: FAKEBOARD,
        last_seen: HashMap::new(),
    };
    let commands = setup();

    println!("Hello, {}!", &node_id);
    help(&state, &commands);
    println!();
    state_describe(conn, &mut state, [].to_vec());

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

        for command in commands.iter() {
            if !(command.available)(&state) {
                continue;
            }
            if let Some(captures) = command.pattern.captures(trimmed) {
                (command.func)(
                    conn,
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
        help(&state, &commands);
    }
}

/// Show the user all commands available to them right now.
fn help(state: &UserState, commands: &Vec<Command>) {
    println!("\nCommands:\n");
    let width = commands
        .iter()
        .filter(|x| (x.available)(state))
        .map(|x| x.arg.len())
        .max()
        .unwrap();
    for command in commands {
        if (command.available)(state) {
            println!("{:width$} : {}", command.arg, command.help);
        }
    }
    println!("{:width$} : Quit", "Q");
}
