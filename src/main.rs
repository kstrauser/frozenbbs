use clap::{Parser, Subcommand};
use frozenbbs::{admin, client, db};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Subsystems>,
}

#[derive(Debug, Subcommand)]
enum Subsystems {
    /// Connect with a client
    Client {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: String,
    },
    /// Admin commands
    #[command(arg_required_else_help = true)]
    Admin {
        #[command(subcommand)]
        admin_command: Option<AdminCommands>,
    },
}

#[derive(Debug, Subcommand)]
enum AdminCommands {
    /// User commands
    #[command(arg_required_else_help = true)]
    User {
        #[command(subcommand)]
        user_command: Option<AdminUserCommands>,
    },
    /// Board commands
    #[command(arg_required_else_help = true)]
    Board {
        #[command(subcommand)]
        board_command: Option<AdminBoardCommands>,
    },
    /// Post commands
    #[command(arg_required_else_help = true)]
    Post {
        #[command(subcommand)]
        post_command: Option<AdminPostCommands>,
    },
}

#[derive(Debug, Subcommand)]
enum AdminUserCommands {
    /// List all users.
    List {},
    /// Add a new user.
    Add {
        /// User's node ID in !hex format.
        #[arg(short, long)]
        node_id: String,
        /// User's 4-byte short name.
        #[arg(short, long)]
        short: String,
        /// User's long name.
        #[arg(short, long)]
        long: String,
        /// The user is a jackass.
        #[arg(short, long)]
        jackass: bool,
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

#[derive(Debug, Subcommand)]
enum AdminBoardCommands {
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
enum AdminPostCommands {
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

fn main() {
    let cli = Cli::parse();

    let connection = &mut db::establish_connection();

    match &cli.command {
        Some(Subsystems::Admin { admin_command }) => match admin_command {
            Some(AdminCommands::User { user_command }) => match user_command {
                Some(AdminUserCommands::List {}) => admin::user_list(connection),
                Some(AdminUserCommands::Add {
                    node_id,
                    short,
                    long,
                    jackass,
                }) => admin::user_add(connection, node_id, short, long, jackass),
                Some(AdminUserCommands::Ban { node_id }) => admin::user_ban(connection, node_id),
                Some(AdminUserCommands::Unban { node_id }) => {
                    admin::user_unban(connection, node_id)
                }
                None => {}
            },
            Some(AdminCommands::Board { board_command }) => match board_command {
                Some(AdminBoardCommands::List {}) => admin::board_list(connection),
                Some(AdminBoardCommands::Add { name, description }) => {
                    admin::board_add(connection, name, description)
                }
                None => {}
            },
            Some(AdminCommands::Post { post_command }) => match post_command {
                Some(AdminPostCommands::Read { board_id }) => {
                    admin::post_read(connection, *board_id)
                }
                Some(AdminPostCommands::Add {
                    board_id,
                    node_id,
                    content,
                }) => admin::post_add(connection, *board_id, node_id, content),
                None => {}
            },
            None => {}
        },
        Some(Subsystems::Client { node_id }) => client::client(connection, node_id),
        None => {}
    }
}
