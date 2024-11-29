use crate::client::dispatch;
use crate::commands::setup;
use crate::db::users;
use crate::node_id_from_hex;
use diesel::SqliteConnection;
use meshtastic::api::StreamApi;
use meshtastic::utils::generate_rand_id;
use meshtastic::utils::stream::build_tcp_stream;
use std::process::Command;

fn bullshit_send(recipient: &str, message: &str) {
    let _ = Command::new("./meshtastic-python")
        .args([
            "--host",
            "localhost",
            "--dest",
            recipient,
            "--sendtext",
            message,
        ])
        .output()
        .expect("Unable to send");
    log::debug!("Sent");
}

pub async fn event_loop(
    conn: &mut SqliteConnection,
    our_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream_api = StreamApi::new();
    let our_id = node_id_from_hex(our_id);
    let entered_address = "localhost:4403".to_string();

    let tcp_stream = build_tcp_stream(entered_address).await?;
    let (mut decoded_listener, stream_api) = stream_api.connect(tcp_stream).await;
    let config_id = generate_rand_id();
    let stream_api = stream_api.configure(config_id).await?;

    let commands = setup();

    // This is really pathetic. I haven't figured out how to send messages with the meshtastic
    // crate. To unblock progress, this currently shells out to a Python program to actually
    // reply to clients. It gets worse: running the external program causes the next .rec().await
    // call to hang forever. My hypothesis is that the radio's tiny TCP stack can't handle
    // multiple simultaneous connections, so when the subprocess runs, this connection gets dropped
    // inside the radio's kernel or something. I dunno. I haven't spent too long troubleshooting
    // because I don't want to keep this horrid workaround in place too long anyway.
    while let Some(decoded) = decoded_listener.recv().await {
        if let Some((node_id, command)) = handle_from_radio_packet(conn, our_id, decoded) {
            log::debug!("Received command from {}: <{}>", node_id, command);
            let result = dispatch(conn, &node_id, &commands, command.trim());
            log::debug!("Result: {}", &result);
            bullshit_send(&node_id, result.trim());
            log::debug!("Back in the loop");
            break;
        }
    }
    let _stream_api = stream_api.disconnect().await?;
    Ok(())
}

fn hex_node(node_num: u32) -> String {
    format!("!{:x}", node_num)
}

fn handle_from_radio_packet(
    conn: &mut SqliteConnection,
    our_id: u32,
    from_radio_packet: meshtastic::protobufs::FromRadio,
) -> Option<(String, String)> {
    match from_radio_packet.payload_variant? {
        meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
            let user = node_info.user?;
            if let Ok((user, seen)) = users::observe(
                conn,
                &user.id,
                &user.short_name,
                &user.long_name,
                node_info.last_heard as i64 * 1_000_000,
            ) {
                if seen {
                    log::debug!("Observed at {}: {}", node_info.last_heard, user);
                } else {
                    log::info!("Observed new at {}: {}", node_info.last_heard, user);
                }
            };
        }
        meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet) => {
            return handle_mesh_packet(mesh_packet, our_id);
        }
        _ => {}
    };
    None
}

fn handle_mesh_packet(
    mesh_packet: meshtastic::protobufs::MeshPacket,
    our_id: u32,
) -> Option<(String, String)> {
    let packet_data = match mesh_packet.payload_variant? {
        meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(decoded_mesh_packet) => {
            decoded_mesh_packet
        }
        _ => {
            return None;
        }
    };

    if packet_data.portnum() != meshtastic::protobufs::PortNum::TextMessageApp {
        return None;
    }

    let decoded_text_message = String::from_utf8(packet_data.payload.clone()).unwrap();
    // DMs: to == radio's own ID, channel = 0
    // Public: to == 0xffffffff
    log::debug!(
        "USER: Received text message packet from {:x} to {:x} in channel {}: {}",
        mesh_packet.from,
        mesh_packet.to,
        mesh_packet.channel,
        decoded_text_message
    );
    if mesh_packet.to != our_id {
        return None;
    }
    Some((hex_node(mesh_packet.from), decoded_text_message))
}
