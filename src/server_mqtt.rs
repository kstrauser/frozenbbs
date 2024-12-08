use crate::{client::dispatch, commands, db::users, hex_id_to_num, num_id_to_hex, BBSConfig};
use diesel::SqliteConnection;
use meshtastic::protobufs::{mesh_packet, PortNum, ServiceEnvelope, User};
use prost::Message;
use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use std::time::Duration;

pub fn event_loop(conn: &mut SqliteConnection, cfg: &BBSConfig) {
    let my_id = hex_id_to_num(&cfg.my_id);
    let commands = commands::setup();

    let mut mqttoptions = MqttOptions::new(&cfg.mqtt_id, &cfg.mqtt_hostname, cfg.mqtt_port);
    mqttoptions.set_credentials(&cfg.mqtt_username, &cfg.mqtt_password);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);
    client
        .subscribe(format!("{}/#", cfg.mqtt_root), QoS::AtMostOnce)
        .unwrap();

    for event in connection.iter().flatten() {
        handle_packet(conn, cfg, &commands, event, my_id);
    }
}

fn handle_packet(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    commands: &Vec<commands::Command>,
    event: Event,
    my_id: u32,
) {
    let incoming = match event {
        Event::Incoming(x) => x,
        _ => return,
    };
    let payload = match incoming {
        Incoming::Publish(x) => x.payload,
        _ => return,
    };
    let decoded_packet = ServiceEnvelope::decode(payload);
    let envelope_packet = match decoded_packet {
        Ok(x) => x.packet,
        _ => return,
    };
    let meshpacket = match envelope_packet {
        Some(x) => x,
        _ => return,
    };
    // dbg!(&meshpacket);
    let variant_packet = match meshpacket.payload_variant {
        Some(x) => x,
        _ => return,
    };
    // dbg!(&variant_packet);
    let variant = match variant_packet {
        mesh_packet::PayloadVariant::Decoded(x) => x,
        _ => return,
    };
    // dbg!(&variant);
    if variant.portnum == PortNum::TextMessageApp as i32 && meshpacket.to == my_id {
        let node_id = num_id_to_hex(meshpacket.from);
        let message = std::str::from_utf8(&variant.payload);
        let command = message.unwrap();
        log::debug!("Received command from {}: <{}>", node_id, command);
        let result = dispatch(conn, &node_id, commands, command.trim());
        log::debug!("Result: {}", &result);
        bullshit_send(cfg, &node_id, result.trim());
    }
    if variant.portnum == PortNum::NodeinfoApp as i32 {
        let user = User::decode(&variant.payload[..]).unwrap();
        if let Ok((bbs_user, seen)) = users::observe(
            conn,
            &user.id,
            &user.short_name,
            &user.long_name,
            meshpacket.rx_time as i64 * 1_000_000,
        ) {
            if seen {
                log::info!("Observed at {}: {}", meshpacket.rx_time, bbs_user);
            } else {
                log::info!("Observed new at {}: {}", meshpacket.rx_time, bbs_user);
            }
        };
    }
}

fn bullshit_send(cfg: &BBSConfig, recipient: &str, message: &str) {
    log::debug!("Sending {} to {}", message, recipient);
    let _ = std::process::Command::new(&cfg.meshtastic_python_path)
        .args([
            "--host",
            &cfg.meshtastic_python_host,
            "--dest",
            recipient,
            "--sendtext",
            message,
            "--ack",
        ])
        .output()
        .expect("Unable to send");
    log::debug!("Sent");
}
