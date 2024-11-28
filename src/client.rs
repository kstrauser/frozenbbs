use crate::commands::{help, setup, Command};
use crate::db::users;
use diesel::SqliteConnection;
use std::io::{self, Write as _};

fn dispatch(conn: &mut SqliteConnection, node_id: &str, commands: &Vec<Command>, cmdline: &str) {
    let (mut user, seen) = users::record(conn, node_id).unwrap();
    if !seen {
        println!("Welcome to Frozen BBS!\n");
        help(&user, commands);
    }
    for command in commands.iter() {
        if !(command.available)(&user) {
            continue;
        }
        if let Some(captures) = command.pattern.captures(cmdline) {
            // Collect all of the matched groups in the pattern into a vector of strs
            let args = captures
                .iter()
                .skip(1)
                .flatten()
                .map(|x| x.as_str().trim())
                .collect();
            print!("{}", (command.func)(conn, &mut user, args));
            return;
        }
    }
    match cmdline.to_lowercase().as_str() {
        "q" => {
            println!("buh-bye!");
            return;
        }
        "h" => {}
        _ => {
            println!("That's not an available command here.\n");
        }
    }
    print!("{}", help(&user, commands));
}

/// Run a session from the local terminal.
pub fn client(conn: &mut SqliteConnection, node_id: &str) {
    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let stdin = io::stdin(); // We get `Stdin` here.
    let commands = setup();

    loop {
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
        let buffer = buffer.trim();
        dispatch(conn, node_id, &commands, buffer);
    }
}
