use clap::{ArgAction, Parser, Subcommand};
use frozenbbs::{
    admin, canonical_node_id, client, config_load, config_path, db, default_db_path, server,
    FAKE_MY_ID,
};
use log::LevelFilter;

// The command line layout

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    #[arg(short,long,action=ArgAction::Count)]
    verbose: u8,

    /// Show detailed version / identity (same as server startup).
    #[arg(long = "whoami", short = 'W')]
    whoami: bool,

    #[command(subcommand)]
    command: Option<Subsystems>,
}

#[derive(Debug, Subcommand)]
enum Subsystems {
    /// Client commands
    #[command(arg_required_else_help = true)]
    Client {
        #[command(subcommand)]
        client_command: Option<ClientCommands>,
    },
    /// Server commands
    Server {},
    /// Config commands
    #[command(arg_required_else_help = true)]
    Config {
        #[command(subcommand)]
        config_command: Option<ConfigCommands>,
    },
    /// Board commands
    #[command(arg_required_else_help = true)]
    Board {
        #[command(subcommand)]
        board_command: Option<BoardCommands>,
    },
    /// Post commands
    #[command(arg_required_else_help = true)]
    Post {
        #[command(subcommand)]
        post_command: Option<PostCommands>,
    },
    /// User commands
    #[command(arg_required_else_help = true)]
    User {
        #[command(subcommand)]
        user_command: Option<UserCommands>,
    },
}

#[derive(Debug, Subcommand)]
enum ClientCommands {
    /// Open a local terminal session.
    Terminal {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: Option<String>,
    },
    /// Run a single command.
    Command {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: Option<String>,
        /// The command to run.
        #[arg()]
        command: String,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    /// Show the path to the config file.
    ConfigPath {},
    /// Show the contents of the config file.
    Dump {},
    /// Show the path to the database file.
    DbPath {},
}

#[derive(Debug, Subcommand)]
enum BoardCommands {
    /// List all boards.
    List {},

    /// Add a new board.
    Add {
        /// Name of the board to add.
        #[arg(short, long)]
        name: String,
        /// Description of the new board.
        #[arg(short, long)]
        description: String,
    },
}

#[derive(Debug, Subcommand)]
enum PostCommands {
    /// Read a board's posts.
    Read {
        /// Number of the board to read.
        #[arg(short, long)]
        board_id: i32,
    },

    /// Add a new post.
    Add {
        /// Number of the board to post to.
        #[arg(short, long)]
        board_id: i32,
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: Option<String>,
        /// Body of the new post.
        #[arg(short, long)]
        content: String,
    },
}

#[derive(Debug, Subcommand)]
enum UserCommands {
    /// List all users.
    List {},
    /// Add a new user as though we see their node info.
    Observe {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: String,
        /// User's 4-byte short name.
        #[arg(short, long)]
        short_name: Option<String>,
        /// User's long name.
        #[arg(short, long)]
        long_name: Option<String>,
    },
    /// Set the user's jackass bit.
    Ban {
        /// User's node ID in !hex format
        #[arg(short, long)]
        node_id: String,
    },
    /// Unset the user's jackass bit.
    Unban {
        /// User's node ID in !hex format
        #[arg(short, long)]
        node_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whoami_flag_is_parsed() {
        let cli = Cli::try_parse_from(["frozenbbs", "-W"]).unwrap();
        assert!(cli.whoami);
        assert_eq!(cli.verbose, 0);
        assert!(cli.command.is_none());
    }

    #[test]
    fn whoami_can_be_combined_with_verbose() {
        let cli = Cli::try_parse_from(["frozenbbs", "-v", "-W"]).unwrap();
        assert_eq!(cli.verbose, 1);
        assert!(cli.whoami);
    }
}

/// The main command line handler.
#[allow(clippy::collapsible_match)]
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.whoami {
        let cfg = config_load().expect("the default configuration should always be available");
        println!("{}", frozenbbs::system_info(&cfg));
        return;
    }

    // Process commands to show configuration information before the system is fully configured.
    if let Some(Subsystems::Config { config_command }) = &cli.command {
        match config_command {
            Some(ConfigCommands::ConfigPath {}) => println!("{}", config_path().display()),
            Some(ConfigCommands::Dump {}) => {
                let cfg = config_load().expect("the system should be configured");
                println!(
                    "{}",
                    toml::to_string(&cfg)
                        .expect("toml should be able to serialize a simple config object")
                );
            }
            Some(ConfigCommands::DbPath {}) => {
                let cfg = config_load();
                if let Ok(cfg) = cfg {
                    println!("{}", &cfg.db_path);
                } else {
                    eprintln!(
                        "Giving the default database path because the config file is missing."
                    );
                    println!("{}", default_db_path().display());
                }
            }
            None => {}
        }
        return;
    };

    let cfg = config_load().expect("the default configuration should always be available");

    // Crank up the BBS and Meshtastic logging as verbosity increases.
    let (bbs_level, radio_level) = match cli.verbose {
        0 => (LevelFilter::Warn, LevelFilter::Off),
        1 => (LevelFilter::Info, LevelFilter::Off),
        2 => (LevelFilter::Debug, LevelFilter::Off),
        3 => (LevelFilter::Debug, LevelFilter::Error),
        4 => (LevelFilter::Debug, LevelFilter::Info),
        _ => (LevelFilter::Debug, LevelFilter::Debug),
    };
    simple_logger::SimpleLogger::new()
        .with_module_level("meshtastic::connections", radio_level)
        .with_level(bbs_level)
        .with_local_timestamps()
        .init()
        .unwrap();

    if cfg.my_id == FAKE_MY_ID {
        eprintln!(
            "!!!!!!!!!
You are using the default value for `my_id`.

Edit \"{}\" to set it to your radio's node ID.
!!!!!!!!",
            config_path().display(),
        );
    }

    let conn = &mut db::establish_connection(&cfg);

    // Use the passed-in node ID, if given, or else the node's own ID.
    let default_or = |node_id: &Option<String>| -> String {
        let id_or_default = if let Some(x) = node_id { x } else { &cfg.my_id };
        canonical_node_id(id_or_default).unwrap()
    };

    match &cli.command {
        Some(Subsystems::Client { client_command }) => match client_command {
            Some(ClientCommands::Terminal { node_id }) => {
                client::terminal(conn, &cfg, &default_or(node_id));
            }
            Some(ClientCommands::Command { node_id, command }) => {
                client::command(conn, &cfg, &default_or(node_id), command);
            }
            None => {}
        },
        Some(Subsystems::Server {}) => server::event_loop(conn, &cfg).await.unwrap(),
        Some(Subsystems::Board { board_command }) => match board_command {
            Some(BoardCommands::List {}) => admin::board_list(conn),
            Some(BoardCommands::Add { name, description }) => {
                admin::board_add(conn, name, description);
            }
            None => {}
        },
        Some(Subsystems::Config { .. }) => {} // Already handled this arm earlier.
        Some(Subsystems::Post { post_command }) => match post_command {
            Some(PostCommands::Read { board_id }) => admin::post_read(conn, *board_id),
            Some(PostCommands::Add {
                board_id,
                node_id,
                content,
            }) => admin::post_add(conn, *board_id, &default_or(node_id), content),
            None => {}
        },
        Some(Subsystems::User { user_command }) => match user_command {
            Some(UserCommands::List {}) => admin::user_list(conn),
            Some(UserCommands::Observe {
                node_id,
                short_name,
                long_name,
            }) => admin::user_observe(
                conn,
                &canonical_node_id(node_id).unwrap(),
                short_name.as_deref(),
                long_name.as_deref(),
            ),
            Some(UserCommands::Ban { node_id }) => {
                admin::user_ban(conn, &canonical_node_id(node_id).unwrap());
            }
            Some(UserCommands::Unban { node_id }) => {
                admin::user_unban(conn, &canonical_node_id(node_id).unwrap());
            }
            None => {}
        },
        None => {}
    }
}
