pub mod admin;
pub mod client;
pub mod commands;
pub mod db;
pub mod radio;

pub fn node_id_from_hex(node_id: &str) -> u32 {
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
