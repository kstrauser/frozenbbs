use clap::{ArgAction, Parser, Subcommand};
use frozenbbs::{admin, client, db, server_serial, BBSConfig, BBS_TAG};
use log::LevelFilter;

// The command line layout

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    #[arg(short,long,action=ArgAction::Count)]
    verbose: u8,
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
    /// Board commands
    #[command(arg_required_else_help = true)]
    Board {
        #[command(subcommand)]
        board_command: Option<BoardCommands>,
    },
    /// Database commands
    #[command(arg_required_else_help = true)]
    Db {
        #[command(subcommand)]
        db_command: Option<DbCommands>,
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
        node_id: String,
    },
    /// Run a single command.
    Command {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: String,
        /// The command to run.
        #[arg()]
        command: String,
    },
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
enum DbCommands {
    /// Show the path to the database file.
    Path {},
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
        node_id: String,
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
        short_name: String,
        /// User's long name.
        #[arg(short, long)]
        long_name: String,
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

/// The main command line handler.
#[allow(clippy::collapsible_match)]
#[tokio::main]
async fn main() {
    let cfg: BBSConfig = confy::load(BBS_TAG, "config").unwrap();
    let cli = Cli::parse();
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
        .init()
        .unwrap();
    let conn = &mut db::establish_connection(&cfg);

    match &cli.command {
        Some(Subsystems::Client { client_command }) => match client_command {
            Some(ClientCommands::Terminal { node_id }) => client::terminal(conn, node_id),
            Some(ClientCommands::Command { node_id, command }) => {
                client::command(conn, node_id, command)
            }
            None => {}
        },
        Some(Subsystems::Server {}) => server_serial::event_loop(conn, &cfg).await.unwrap(),
        Some(Subsystems::Board { board_command }) => match board_command {
            Some(BoardCommands::List {}) => admin::board_list(conn),
            Some(BoardCommands::Add { name, description }) => {
                admin::board_add(conn, name, description)
            }
            None => {}
        },
        Some(Subsystems::Db { db_command }) => match db_command {
            Some(DbCommands::Path {}) => admin::db_path(cfg),
            None => {}
        },
        Some(Subsystems::Post { post_command }) => match post_command {
            Some(PostCommands::Read { board_id }) => admin::post_read(conn, *board_id),
            Some(PostCommands::Add {
                board_id,
                node_id,
                content,
            }) => admin::post_add(conn, *board_id, node_id, content),
            None => {}
        },
        Some(Subsystems::User { user_command }) => match user_command {
            Some(UserCommands::List {}) => admin::user_list(conn),
            Some(UserCommands::Observe {
                node_id,
                short_name,
                long_name,
            }) => admin::user_observe(conn, node_id, short_name, long_name),
            Some(UserCommands::Ban { node_id }) => admin::user_ban(conn, node_id),
            Some(UserCommands::Unban { node_id }) => admin::user_unban(conn, node_id),
            None => {}
        },
        None => {}
    }
}
