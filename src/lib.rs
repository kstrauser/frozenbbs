pub mod admin;
pub mod client;
pub mod commands;
pub mod db;
pub mod paginate;
pub mod server;
pub mod server_mqtt;
use serde::{Deserialize, Serialize};

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
    my_id: String,
    mqtt_root: String,
    // Identifies this program to the MQTT broker
    mqtt_id: String,
    mqtt_hostname: String,
    mqtt_port: u16,
    mqtt_username: String,
    mqtt_password: String,
    meshtastic_python_path: String,
    meshtastic_python_host: String,
}

impl ::std::default::Default for BBSConfig {
    fn default() -> Self {
        Self {
            my_id: "cafeb33d".into(),
            mqtt_root: "msh".into(),
            mqtt_id: "frozenbbs".into(),
            mqtt_hostname: "localhost".into(),
            mqtt_port: 1883,
            mqtt_username: "meshdev".into(),
            mqtt_password: "large4cats".into(),
            meshtastic_python_path: "./meshtastic-python".into(),
            meshtastic_python_host: "localhost".into(),
        }
    }
}
