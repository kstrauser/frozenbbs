pub mod admin;
pub mod client;
pub mod commands;
pub mod db;
pub mod paginate;
pub mod server;
use config::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    format!("!{node_num:x}")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BBSConfig {
    bbs_name: String,
    pub my_id: String,
    pub db_path: String,
    serial_device: Option<String>,
    tcp_address: Option<String>,
    sysops: Vec<String>,
    public_channel: u32,
    ad_text: String,
}

pub fn config_path() -> PathBuf {
    let xdg_dirs = xdg::BaseDirectories::with_prefix(BBS_TAG).unwrap();
    xdg_dirs
        .place_config_file("config.toml")
        .expect("Unable to create the config path")
}

pub fn default_db_path() -> PathBuf {
    let xdg_dirs = xdg::BaseDirectories::with_prefix(BBS_TAG).unwrap();
    xdg_dirs
        .place_data_file(format!("{BBS_TAG}.db"))
        .expect("Unable to create the database file path")
}

pub fn load_config() -> BBSConfig {
    let config_path = config_path();

    let config = Config::builder()
        .add_source(config::File::from(config_path.clone()))
        .build();
    if let Ok(config) = config {
        let config: BBSConfig = config.try_deserialize().unwrap();

        if (config.serial_device.is_some() && config.tcp_address.is_some())
            || (config.serial_device.is_none() && config.tcp_address.is_none())
        {
            panic!("Exactly one of serial_device or tcp_device must be configured.");
        }

        return config;
    }

    let config = BBSConfig {
        bbs_name: "Frozen BBS❅".into(),
        my_id: "!cafeb33d".into(),
        db_path: default_db_path().into_os_string().into_string().unwrap(),
        serial_device: Some("/dev/ttyUSB0".into()),
        tcp_address: None,
        sysops: Vec::new(),
        public_channel: 0,
        ad_text: "I'm running a BBS on this node. DM me to get started!".into(),
    };

    let config = toml::to_string(&config).unwrap();
    let config = config.trim();

    panic!(
        "\
Unable to read the config file at {config_path:?}

Create a new file with values like:

=======
{config}
======="
    );
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
