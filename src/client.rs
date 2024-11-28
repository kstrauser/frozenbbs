use crate::db::{board_states, boards, posts, users, Post, User};
use diesel::SqliteConnection;
use regex::{Regex, RegexBuilder};
use std::io::{self, Write as _};

const NOT_IN_BOARD: &str = "You are not in a board.\n";

fn board_lister(conn: &mut SqliteConnection, _user: &mut User, _args: Vec<&str>) -> String {
    let mut out = String::new();
    out.push_str("Boards:\n\n");
    let all_boards = boards::all(conn);
    if all_boards.is_empty() {
        out.push_str("There are no boards.\n");
    } else {
        for board in boards::all(conn) {
            out.push_str(&format!(
                "#{} {}: {}\n",
                board.id, board.name, board.description
            ));
        }
    }
    out
}

fn board_enter(conn: &mut SqliteConnection, user: &mut User, args: Vec<&str>) -> String {
    let num = match args[0].parse::<i32>() {
        Ok(num) => num,
        Err(_) => {
            return "Not a valid number!\n".to_string();
        }
    };
    let count = boards::count(conn);
    if count == 0 {
        return "There are no boards.\n".to_string();
    }
    if num < 1 || num > count {
        return format!("Board number must be between 1 and {}\n", count);
    }
    let _ = users::enter_board(conn, user, num);
    format!("Entering board {}\n", num)
}

/// Print a post and information about its author.
fn post_print(post: &Post, user: &User) -> String {
    let mut out = String::new();
    out.push_str(&format!("From: {}\n", user));
    out.push_str(&format!("At  : {}\n", post.created_at()));
    out.push_str(&format!("Msg : {}\n", post.body));
    out
}

fn board_previous(conn: &mut SqliteConnection, user: &mut User, _args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.to_string();
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
        "There are no more posts in this board.\n".to_string()
    }
}

fn board_next(conn: &mut SqliteConnection, user: &mut User, _args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.to_string();
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
        "There are no more posts in this board.\n".to_string()
    }
}

fn board_write(conn: &mut SqliteConnection, user: &mut User, args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.to_string();
        }
    };
    let post = posts::add(conn, user.id, in_board, args[0]).unwrap();
    format!("Published at {}.\n", post.created_at())
}

fn state_describe(conn: &mut SqliteConnection, user: &mut User, _args: Vec<&str>) -> String {
    let in_board = match user.in_board {
        Some(v) => v,
        None => {
            return NOT_IN_BOARD.to_string();
        }
    };
    let board = boards::get(conn, in_board).unwrap();
    format!("You are in board #{}: {}\n", in_board, board.name)
}

/// Show the user all commands available to them right now.
fn help(user: &User, commands: &Vec<Command>) -> String {
    let mut out = String::new();
    out.push_str("\nCommands:\n\n");
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
    out.push_str(&format!("{:width$} : Quit\n", "Q"));
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
struct Command {
    /// Help text showing the user what to send.
    arg: String,
    /// What the command does.
    help: String,
    /// The pattern matching the command and its arguments.
    pattern: Regex,
    /// A function that determines whether the user in this state can run this command.
    available: fn(&User) -> bool,
    /// The function that implements this command.
    func: fn(&mut SqliteConnection, &mut User, Vec<&str>) -> String,
}

/// Build a Regex in our common fashion.
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
    let commands = setup();

    let mut this_user = users::get(conn, node_id).unwrap();
    println!("{}", help(&this_user, &commands));
    println!();
    state_describe(conn, &mut this_user, [].to_vec());

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
        users::saw(conn, &this_user.node_id);
        let trimmed = buffer.trim();
        let lower = trimmed.to_lowercase();

        let mut this_user = users::get(conn, node_id).unwrap();

        for command in commands.iter() {
            if !(command.available)(&this_user) {
                continue;
            }
            if let Some(captures) = command.pattern.captures(trimmed) {
                // Collect all of the matched groups in the pattern into a vector of strs
                let args = captures
                    .iter()
                    .skip(1)
                    .flatten()
                    .map(|x| x.as_str().trim())
                    .collect();
                print!("{}", (command.func)(conn, &mut this_user, args));
                continue 'outer;
            }
        }

        if lower == "q" {
            println!("buh-bye!");
            return;
        }

        println!("That's not an available command here.");
        println!("{}", help(&this_user, &commands));
    }
}
