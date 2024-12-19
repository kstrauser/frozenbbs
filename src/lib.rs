pub mod admin;
pub mod client;
pub mod commands;
pub mod db;
pub mod paginate;
pub mod server_serial;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const BBS_TAG: &str = "frozenbbs";

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
    bbs_name: String,
    pub my_id: String,
    db_path: String,
    serial_device: String,
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
        let db_path = Path::new(&db_file);

        Self {
            bbs_name: "Frozen BBS❅".to_string(),
            my_id: "cafeb33d".into(),
            db_path: data_home.join(db_path).to_str().unwrap().to_owned(),
            serial_device: "/dev/ttyUSB0".into(),
        }
    }
}
