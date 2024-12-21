pub mod admin;
pub mod client;
pub mod commands;
pub mod db;
pub mod paginate;
pub mod server_serial;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const BBS_TAG: &str = "frozenbbs";

/// Convert a node Id like 12345678 or !abcdef12 to their u32 value.
pub fn hex_id_to_num(node_id: &str) -> u32 {
    u32::from_str_radix(
        if node_id.starts_with("!") {
            node_id.strip_prefix("!").unwrap()
        } else {
            node_id
        },
        16,
    )
    .unwrap()
}

/// Convert a u32 node ID to its canonical !abcdef12 format.
pub fn num_id_to_hex(node_num: u32) -> String {
    format!("!{:x}", node_num)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BBSConfig {
    bbs_name: String,
    pub my_id: String,
    db_path: String,
    serial_device: String,
    sysops: Vec<String>,
    public_channel: u32,
    ad_text: String,
}

impl ::std::default::Default for BBSConfig {
    fn default() -> Self {
        eprintln!(
            "\
NOTICE!

Creating a new config file at \"{}\".

Edit it before doing anything else!

===================================
",
            confy::get_configuration_file_path(BBS_TAG, "config")
                .unwrap()
                .display()
        );
        let xdg_dirs = xdg::BaseDirectories::with_prefix(BBS_TAG).unwrap();
        let data_home = xdg_dirs.get_data_home();
        let data_home = Path::new(&data_home);
        let db_file = format!("{}.db", BBS_TAG);
        let db_filename = Path::new(&db_file);
        let db_path = data_home.join(db_filename).to_str().unwrap().to_owned();

        Self {
            bbs_name: "Frozen BBS❅".into(),
            my_id: "!cafeb33d".into(),
            db_path,
            serial_device: "/dev/ttyUSB0".into(),
            sysops: Vec::new(),
            public_channel: 0,
            ad_text: "I'm running a BBS on this node. DM me to get started!".into(),
        }
    }
}

/// Describe this system.
pub fn system_info(cfg: &BBSConfig) -> String {
    format!(
        "{} is running {} v{}/{} built at {}.",
        cfg.bbs_name,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("VERGEN_GIT_DESCRIBE"),
        &env!("VERGEN_BUILD_TIMESTAMP").to_string()[..22],
    )
}

/// Add an empty line to the output.
#[macro_export]
macro_rules! linefeed {
    ($x:expr) => {
        $x.push("".to_string());
    };
}
