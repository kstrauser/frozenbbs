use crate::db::User;
use crate::{linefeed, BBSConfig};
use diesel::SqliteConnection;
use regex::{Regex, RegexBuilder};
mod board;
mod dm;
mod state;
mod sysop;
mod user;

const ERROR_POSTING: &str = "Unable to insert this post.";

/// To where shall I respond?
#[derive(Debug)]
pub enum ReplyDestination {
    Sender,
    Broadcast,
}

/// Where and what to send back to the radio.
#[derive(Debug)]
pub struct Reply {
    pub out: Vec<String>,
    pub destination: ReplyDestination,
}

/// The collection of reply messages that a command returns to the client.
#[derive(Debug)]
pub struct Replies(pub Vec<Reply>);

/// The command returns a whole Vec of Strings.
impl From<Vec<String>> for Replies {
    fn from(out: Vec<String>) -> Self {
        Replies(vec![Reply {
            out,
            destination: ReplyDestination::Sender,
        }])
    }
}

/// The command returns a single &str.
impl From<&str> for Replies {
    fn from(out: &str) -> Self {
        Replies(vec![Reply {
            out: vec![out.to_string()],
            destination: ReplyDestination::Sender,
        }])
    }
}

/// The command returns a single String.
impl From<String> for Replies {
    fn from(out: String) -> Self {
        Replies(vec![Reply {
            out: vec![out],
            destination: ReplyDestination::Sender,
        }])
    }
}

// Help creators

/// Show the user how to get help on all menus available to them right now.
pub fn help_toplevel(state: &AvailableState, menus: &Menus) -> Vec<String> {
    let mut out = Vec::new();
    out.push("Help commands:".to_string());
    linefeed!(out);
    for menu in menus {
        if menu.any_available(state) {
            out.push(format!("H{} : {} menu", menu.help_suffix, menu.name));
        }
    }
    out.push("H : This help".to_string());
    out
}

/// Show the user the commands available to them on this menu.
pub fn help_menu(state: &AvailableState, menu: &Menu) -> Vec<String> {
    let mut out = vec![format!("Help for {} commands", menu.name)];
    linefeed!(out);
    for command in &menu.commands {
        if (command.available)(state) {
            out.push(format!("{} : {}", command.arg, command.help));
        }
    }
    out
}

// Contexts in which certain actions may be available

/// Information about the user's state during a single command.
pub struct AvailableState {
    in_board: bool,
    is_local: bool,
    is_sysop: bool,
}

/// Pre-compute values used by available_* functions so we're not repeatedly hitting the database.
pub fn available_state(cfg: &BBSConfig, user: &User, local: bool) -> AvailableState {
    AvailableState {
        in_board: user.in_board.is_some(),
        is_local: local,
        is_sysop: cfg.sysops.contains(&user.node_id),
    }
}

/// These commands are always available.
fn available_always(_state: &AvailableState) -> bool {
    true
}

/// These commands are available to local users.
fn available_locally(state: &AvailableState) -> bool {
    state.is_local
}

/// These commands are available to sysops.
fn available_to_sysops(state: &AvailableState) -> bool {
    state.is_sysop
}

/// These commands are available when the user is in a message board.
fn available_in_board(state: &AvailableState) -> bool {
    state.in_board
}

// Build the collection of defined commands

/// Collections of BBS commands.
pub struct Menu {
    pub name: String,
    pub help_suffix: String,
    pub commands: Vec<Command>,
}

impl Menu {
    /// Are any commands in this section available to the user?
    pub fn any_available(&self, state: &AvailableState) -> bool {
        self.commands.iter().any(|x| (x.available)(state))
    }
}

pub type Menus = Vec<Menu>;

/// Information about a command a user can execute.
pub struct Command {
    /// Help text showing the user what to send.
    arg: String,
    /// What the command does.
    help: String,
    /// The pattern matching the command and its arguments.
    pub pattern: Regex,
    /// A function that determines whether the user in this state can run this command.
    pub available: fn(&AvailableState) -> bool,
    /// The function that implements this command.
    pub func: fn(&mut SqliteConnection, &BBSConfig, &mut User, Vec<&str>) -> Replies,
}

/// Build a Regex in our common fashion.
fn make_pattern(pattern: &str) -> Regex {
    RegexBuilder::new(format!(r"^\s*{pattern}\s*$").as_str())
        .case_insensitive(true)
        .build()
        .unwrap()
}

/// Build a set of menus and their commands from the configuration.
pub fn command_structure(cfg: &BBSConfig) -> Menus {
    let mut menus = Vec::new();

    for (name, menu) in cfg.menus.iter() {
        let commands: Vec<Command> = menu
            .commands
            .iter()
            .map(|command| Command {
                arg: command.arg.clone(),
                help: command.help.clone(),
                pattern: make_pattern(&command.pattern),
                available: match command.available.as_str() {
                    "always" => available_always,
                    "in_board" => available_in_board,
                    "local" => available_locally,
                    "sysop" => available_to_sysops,
                    _ => panic!("Unknown command availability: {}", command.available),
                },
                func: match command.func.as_str() {
                    "board::author" => board::author,
                    "board::current" => board::current,
                    "board::enter" => board::enter,
                    "board::lister" => board::lister,
                    "board::next" => board::next,
                    "board::previous" => board::previous,
                    "board::quick" => board::quick,
                    "board::write" => board::write,
                    "dm::send" => dm::send,
                    "state::describe" => state::describe,
                    "sysop::advertise" => sysop::advertise,
                    "user::active" => user::active,
                    "user::bio_read" => user::bio_read,
                    "user::bio_write" => user::bio_write,
                    "user::seen" => user::seen,
                    _ => panic!("Unknown command function: {}", command.func),
                },
            })
            .collect();

        let menu = Menu {
            name: name.to_string(),
            help_suffix: menu.help_suffix.clone(),
            commands,
        };

        menus.push(menu);
    }

    menus
}
