use crate::db::{boards, posts};
use crate::db::{Post, User};
use crate::formatted_useconds;
use diesel::SqliteConnection;
use std::collections::HashMap;
use std::io::{self, Write as _};

const FAKEBOARD: i32 = -1;

/// The user's global state
#[derive(Debug)]
struct UserState {
    board: i32,
    last_seen: HashMap<i32, i64>,
}

mod board_list {
    const COMMAND: &str = "b";
    const HELP: &str = "Board list";

    fn available(state: &super::UserState) -> bool {
        true
    }

    fn matches(cmd: &str) -> bool {
        cmd.starts_with("b") && cmd.len() == 1
    }

    fn execute(conn: &mut super::SqliteConnection, mut state: &super::UserState, cmd: &str) {
        println!("Boards:");
        println!();
        for board in super::boards::all(conn) {
            println!("#{} {}: {}", board.id, board.name, board.description);
        }
    }
}

pub fn client(connection: &mut SqliteConnection, node_id: &str) {
    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let stdin = io::stdin(); // We get `Stdin` here.
    let mut state = UserState {
        board: FAKEBOARD,
        last_seen: HashMap::new(),
    };
    dbg!(&node_id);

    help(&state);

    loop {
        println!();
        if state.board != FAKEBOARD {
            let board = boards::get(connection, state.board).unwrap();
            println!("You are in board #{}: {}", state.board, board.name);
            println!();
        }
        print!("Command: ");
        stdout.flush().unwrap();
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        println!();
        if buffer.is_empty() {
            println!("Disconnected.");
            return;
        }
        let trimmed = buffer.trim();
        let lower = trimmed.to_lowercase();
        let lower = lower.as_str();

        if lower == "q" {
            println!("buh-bye!");
            return;
        }

        if lower == "b" {
            println!("Boards:");
            println!();
            for board in boards::all(connection) {
                println!("#{} {}: {}", board.id, board.name, board.description);
            }
            continue;
        }

        if let Some(num) = lower.strip_prefix('b') {
            let max_board = boards::all(connection)
                .into_iter()
                .map(|x| x.id)
                .max()
                .unwrap();
            let num = match num.trim().parse::<i32>() {
                Ok(num) => num,
                Err(_) => {
                    println!("Not a valid number!");
                    continue;
                }
            };
            if num < 1 || num > max_board {
                println!("Board number must be between 1 and {}", max_board);
                continue;
            }
            println!("Entering board {}", num);
            state.board = num;
            state.last_seen.insert(num, 0);
            dbg!(&state);
            continue;
        }

        if lower == "n" {
            if let Ok((post, user)) = posts::after(
                connection,
                state.board,
                *state.last_seen.get(&state.board).unwrap(),
            ) {
                post_print(&post, &user);
                state.last_seen.insert(state.board, post.created_at_us);
            } else {
                println!("There are no more posts in this board.");
            }
            continue;
        }

        if lower == "p" {
            if let Ok((post, user)) = posts::before(
                connection,
                state.board,
                *state.last_seen.get(&state.board).unwrap(),
            ) {
                post_print(&post, &user);
                state.last_seen.insert(state.board, post.created_at_us);
            } else {
                println!("There are no more posts in this board.");
            }
            continue;
        }

        println!("Unknown command.");
        help(&state);
    }
}

fn post_print(post: &Post, user: &User) {
    println!(
        "From: {}/{}:{}",
        user.node_id, user.short_name, user.long_name
    );
    println!("At  : {}", formatted_useconds(post.created_at_us));
    println!("Msg : {}", post.body);
}

fn help(state: &UserState) {
    println!(
        "
Commands:

B : Board list
Bn: Enter board #n"
    );
    if state.board != FAKEBOARD {
        println!(
            "\
P : Read the previous message in the board
N : Read the next message in the board"
        );
    }
    println!(
        "\
Q : Quit"
    );
}
