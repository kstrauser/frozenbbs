pub mod admin;
pub mod client;
pub mod commands;
pub mod db;
pub mod paginate;
pub mod server_serial;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

fn num_id_to_hex(node_num: u32) -> String {
    format!("!{:x}", node_num)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BBSConfig {
    db_path: String,
    my_id: String,
    serial_device: String,
}

impl ::std::default::Default for BBSConfig {
    fn default() -> Self {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("frozenbbs").unwrap();
        let data_home = xdg_dirs.get_data_home();
        let data_home = Path::new(&data_home);
        let db_name = Path::new("frozen.db");

        Self {
            db_path: data_home.join(db_name).to_str().unwrap().to_owned(),
            my_id: "cafeb33d".into(),
            serial_device: "/dev/ttyUSB0".into(),
        }
    }
}
