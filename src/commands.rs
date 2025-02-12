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

pub fn command_structure() -> Menus {
    let general_menu = Menu {
        name: "General".to_string(),
        help_suffix: "G".to_string(),
        commands: vec![
            Command {
                arg: "?".to_string(),
                help: "Who and where am I?".to_string(),
                pattern: make_pattern(r"\?"),
                available: available_always,
                func: state::state_describe,
            },
            Command {
                arg: "U".to_string(),
                help: "Recently active users".to_string(),
                pattern: make_pattern("u"),
                available: available_always,
                func: user::user_active,
            },
            Command {
                arg: "S".to_string(),
                help: "Recently seen users".to_string(),
                pattern: make_pattern("s"),
                available: available_always,
                func: user::user_seen,
            },
            Command {
                arg: "DM user msg".to_string(),
                help: "Send a message".to_string(),
                pattern: make_pattern(r"(?s)dm\s*(\S+)\s+(.+?)\s*"),
                available: available_always,
                func: dm::direct_message,
            },
            Command {
                arg: "BIO".to_string(),
                help: "Show your bio".to_string(),
                pattern: make_pattern("bio"),
                available: available_always,
                func: user::user_bio_read,
            },
            Command {
                arg: "BIO msg".to_string(),
                help: "Update your bio".to_string(),
                pattern: make_pattern(r"(?s)bio\s*(.+?)\s*"),
                available: available_always,
                func: user::user_bio_write,
            },
        ],
    };

    let board_menu = Menu {
        name: "Board".to_string(),
        help_suffix: "B".to_string(),
        commands: vec![
            Command {
                arg: "B".to_string(),
                help: "Board list".to_string(),
                pattern: make_pattern("b"),
                available: available_always,
                func: board::board_lister,
            },
            Command {
                arg: "Bn".to_string(),
                help: "Enter board #n".to_string(),
                pattern: make_pattern(r"b\s*(\d+)"),
                available: available_always,
                func: board::board_enter,
            },
            Command {
                arg: "Q".to_string(),
                help: "Read the next unread message in any board".to_string(),
                pattern: make_pattern("q"),
                available: available_always,
                func: board::board_quick,
            },
            Command {
                arg: "P".to_string(),
                help: "Read the previous message".to_string(),
                pattern: make_pattern("p"),
                available: available_in_board,
                func: board::board_previous,
            },
            Command {
                arg: "R".to_string(),
                help: "Read the current message".to_string(),
                pattern: make_pattern("r"),
                available: available_in_board,
                func: board::board_current,
            },
            Command {
                arg: "N".to_string(),
                help: "Read the next message".to_string(),
                pattern: make_pattern("n"),
                available: available_in_board,
                func: board::board_next,
            },
            Command {
                arg: "W msg".to_string(),
                help: "Write a new message".to_string(),
                pattern: make_pattern(r"(?s)w\s*(.+?)\s*"),
                available: available_in_board,
                func: board::board_write,
            },
            Command {
                arg: "BA".to_string(),
                help: "Show the current message's author.".to_string(),
                pattern: make_pattern("ba"),
                available: available_in_board,
                func: board::board_author,
            },
        ],
    };

    let local_menu = Menu {
        name: "Local".to_string(),
        help_suffix: "L".to_string(),
        commands: vec![Command {
            arg: "LA".to_string(),
            help: "Send an advertisement to the public channel.".to_string(),
            pattern: make_pattern("la"),
            available: available_locally,
            func: sysop::sysop_advertise,
        }],
    };

    let sysop_menu = Menu {
        name: "Sysop".to_string(),
        help_suffix: "!".to_string(),
        commands: vec![Command {
            arg: "!A".to_string(),
            help: "Send an advertisement to the public channel.".to_string(),
            pattern: make_pattern("!a"),
            available: available_to_sysops,
            func: sysop::sysop_advertise,
        }],
    };

    vec![general_menu, local_menu, board_menu, sysop_menu]
}
