use crate::commands::{help, setup, Command};
use crate::db::users;
use crate::{system_info, BBSConfig};
use diesel::SqliteConnection;
use std::io::{self, Write as _};

/// Handle a single command from a client and return its output.
pub fn dispatch(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    node_id: &str,
    commands: &Vec<Command>,
    cmdline: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    let (mut user, seen) = users::record(conn, node_id).unwrap();
    if seen {
        log::info!("Command from {}: '{}'", user, cmdline);
    } else {
        log::info!("Command from new {}: '{}'", user, cmdline);
        out.push(format!("Welcome to {}!\n", cfg.bbs_name));
        out.push(system_info(cfg));
        out.push("".to_string());
        out.extend(help(cfg, &user, commands));
        out.push("".to_string());
    }
    for command in commands.iter() {
        if !(command.available)(&user, cfg) {
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
            out.extend((command.func)(conn, cfg, &mut user, args));
            return out;
        }
    }
    if !seen {
        return out;
    }
    match cmdline.to_lowercase().as_str() {
        "h" => {}
        _ => {
            out.push("That's not an available command here.\n".to_string());
        }
    }
    out.extend(help(cfg, &user, commands));
    out
}

/// Run a session from the local terminal.
pub fn terminal(conn: &mut SqliteConnection, cfg: &BBSConfig, node_id: &str) {
    let mut stdout = io::stdout();
    let mut command = String::new();
    let stdin = io::stdin();
    let commands = setup();

    println!("Connected. ^D to quit.");

    loop {
        println!();
        print!("Command: ");
        stdout.flush().unwrap();
        command.clear();
        stdin.read_line(&mut command).unwrap();
        println!();
        if command.is_empty() {
            println!("Disconnected.");
            return;
        }
        print!(
            "{}",
            dispatch(conn, cfg, node_id, &commands, command.trim()).join("\n")
        );
    }
}

/// Run a single command.
pub fn command(conn: &mut SqliteConnection, cfg: &BBSConfig, node_id: &str, command: &str) {
    println!(
        "{}",
        dispatch(conn, cfg, node_id, &setup(), command).join("\n")
    );
}
