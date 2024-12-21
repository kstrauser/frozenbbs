use crate::commands::{command_structure, help_menu, help_toplevel, Menus, Reply};
use crate::db::users;
use crate::{linefeed, system_info, BBSConfig};
use diesel::SqliteConnection;
use std::io::{self, Write as _};

const NO_SUCH_HELP: &str = "That help section does not exist or is not available.";

/// Handle a single command from a client and return its output.
pub fn dispatch(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    node_id: &str,
    menus: &Menus,
    cmdline: &str,
) -> Reply {
    let mut out = Vec::new();
    let (mut user, seen) = users::record(conn, node_id).unwrap();
    if seen {
        log::info!("Command from {}: '{}'", user, cmdline);
    } else {
        log::info!("Command from new {}: '{}'", user, cmdline);
        out.push(format!("Welcome to {}!", cfg.bbs_name));
        linefeed!(out);
        out.push(system_info(cfg));
        linefeed!(out);
        out.extend(help_toplevel(cfg, &user, menus));
        return out.into();
    }

    let cmdline = cmdline.to_lowercase();
    let cmdline = cmdline.as_str();

    // Special handling for help requests
    if cmdline.starts_with("h") {
        let help_suffix = cmdline.strip_prefix("h").unwrap();
        for menu in menus {
            if menu.help_suffix.to_lowercase() == help_suffix {
                if menu.any_available(cfg, &user) {
                    return help_menu(cfg, &user, menu).into();
                }
                break;
            }
        }
        let mut out = Vec::new();
        if !help_suffix.is_empty() {
            out.push(NO_SUCH_HELP.to_string());
            linefeed!(out);
        }
        out.extend(help_toplevel(cfg, &user, menus));
        return out.into();
    }

    for menu in menus {
        for command in menu.commands.iter() {
            if !(command.available)(cfg, &user) {
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
                let response = (command.func)(conn, cfg, &mut user, args);
                out.extend(response.out);
                return Reply {
                    out,
                    destination: response.destination,
                };
            }
        }
    }
    out.push("That's not an available command here.".to_string());
    linefeed!(out);
    out.extend(help_toplevel(cfg, &user, menus));
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
        print!(
            "{}",
            dispatch(conn, cfg, node_id, &commands, command.trim())
                .out
                .join("\n")
        );
    }
}

/// Run a single command.
pub fn command(conn: &mut SqliteConnection, cfg: &BBSConfig, node_id: &str, command: &str) {
    println!(
        "{}",
        dispatch(conn, cfg, node_id, &command_structure(), command)
            .out
            .join("\n")
    );
}
