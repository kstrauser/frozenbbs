use crate::commands::{help, setup, state_describe};
use crate::db::users;
use diesel::SqliteConnection;
use std::io::{self, Write as _};

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
    print!("{}", help(&this_user, &commands));
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
        let buffer = buffer.trim();
        let mut this_user = users::get(conn, node_id).unwrap();
        users::saw(conn, &this_user.node_id);
        for command in commands.iter() {
            if !(command.available)(&this_user) {
                continue;
            }
            if let Some(captures) = command.pattern.captures(buffer) {
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
        match buffer.to_lowercase().as_str() {
            "q" => {
                println!("buh-bye!");
                return;
            }
            "h" => {}
            _ => {
                println!("That's not an available command here.\n");
            }
        }
        print!("{}", help(&this_user, &commands));
    }
}
