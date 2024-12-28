use crate::commands::{
    available_state, command_structure, help_menu, help_toplevel, Menus, Replies, ReplyDestination,
};
use crate::db::users;
use crate::paginate::{paginate, MAX_LENGTH};
use crate::{linefeed, system_info, BBSConfig};
use diesel::SqliteConnection;
use std::io::{self, Write as _};

const NO_SUCH_COMMAND: &str = "That's not an available command here.";
const NO_SUCH_HELP: &str = "That help section does not exist or is not available.";

/// Handle a single command from a client and return its output.
pub fn dispatch(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    node_id: &str,
    menus: &Menus,
    cmdline: &str,
) -> Replies {
    let (mut user, seen) = users::record(conn, node_id).unwrap();
    if seen {
        log::info!("Command from {user}: '{cmdline}'");
    } else {
        log::info!("Command from new {user}: '{cmdline}'");
        let mut out = vec![
            format!("Welcome to {}!", cfg.bbs_name),
            String::new(),
            system_info(cfg),
            String::new(),
        ];
        let state = available_state(cfg, &user);
        out.extend(help_toplevel(&state, menus));
        return out.into();
    }

    let state = available_state(cfg, &user);

    // Special handling for help requests
    let help_cmdline = cmdline.to_lowercase();
    let help_cmdline = help_cmdline.as_str();
    if help_cmdline.starts_with("h") {
        let help_suffix = help_cmdline.strip_prefix("h").unwrap();
        for menu in menus {
            if menu.help_suffix.to_lowercase() == help_suffix {
                // Only acknowledge menus where the user has access to at least one command.
                if menu.any_available(&state) {
                    return help_menu(&state, menu).into();
                }
                break;
            }
        }
        let mut out = Vec::new();
        if !help_suffix.is_empty() {
            // They tried to find a specific help menu but it didn't exist or they don't have
            // access.
            out.push(NO_SUCH_HELP.to_string());
            linefeed!(out);
        }
        out.extend(help_toplevel(&state, menus));
        return out.into();
    }

    for menu in menus {
        for command in &menu.commands {
            // Skip right over commands the user doesn't have access to.
            if !(command.available)(&state) {
                continue;
            }
            if let Some(captures) = command.pattern.captures(cmdline) {
                // Collect all of the matched groups in the pattern into a vector of strs
                let args = captures
                    .iter()
                    .skip(1) // The first item is the command
                    .flatten()
                    .map(|x| x.as_str().trim())
                    .collect();
                return (command.func)(conn, cfg, &mut user, args);
            }
        }
    }

    let mut out = vec![NO_SUCH_COMMAND.to_string(), String::new()];
    out.extend(help_toplevel(&state, menus));
    out.into()
}

/// Run a session from the local terminal.
pub fn terminal(conn: &mut SqliteConnection, cfg: &BBSConfig, node_id: &str) {
    let mut stdout = io::stdout();
    let mut command = String::new();
    let stdin = io::stdin();
    let commands = command_structure();

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
        print_replies(dispatch(conn, cfg, node_id, &commands, command.trim()));
    }
}

/// Run a single command.
pub fn command(conn: &mut SqliteConnection, cfg: &BBSConfig, node_id: &str, command: &str) {
    print_replies(dispatch(conn, cfg, node_id, &command_structure(), command));
}

fn print_replies(replies: Replies) {
    for reply in replies.0 {
        match reply.destination {
            ReplyDestination::Broadcast => {
                println!("Reply to the public channel:");
            }
            ReplyDestination::Sender => {
                println!("Reply to you:");
            }
        };
        println!("{}", paginate(reply.out, MAX_LENGTH).join("\n"));
    }
}
