use crate::db::{boards, posts};
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

pub fn client(connection: &mut SqliteConnection, node_id: &str) {
    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let stdin = io::stdin(); // We get `Stdin` here.
    let mut state = UserState {
        board: FAKEBOARD,
        last_seen: HashMap::new(),
    };
    dbg!(&node_id);

    help();

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

        if lower == "r" {
            if let Ok((post, user)) = posts::after(
                connection,
                state.board,
                *state.last_seen.get(&state.board).unwrap(),
            ) {
                dbg!((&post, &user));
                state.last_seen.insert(state.board, post.created_at_us);
            } else {
                println!("There are no more posts in this board.");
            }
            continue;
        }

        println!("Unknown command.");
        help();
    }
}

fn help() {
    println!(
        "
Commands:

B : Board list
Bn: Enter board #n
Q : Quit"
    );
}
