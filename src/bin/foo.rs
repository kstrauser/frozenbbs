use meshtastic::protobufs::{mesh_packet, Data, NodeInfo, PortNum, ServiceEnvelope, User};
use prost::Message;
use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use std::thread;
use std::time::Duration;

fn main() {
    let mut mqttoptions = MqttOptions::new("rumqtt-sync", "lottie", 1883);
    mqttoptions.set_credentials("meshdev", "large4cats");
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (mut client, mut connection) = Client::new(mqttoptions, 10);
    client.subscribe("msh/#", QoS::AtMostOnce).unwrap();
    // thread::spawn(move || {
    //     for i in 0..10 {
    //         client
    //             .publish("hello/rumqtt", QoS::AtLeastOnce, false, vec![i; i as usize])
    //             .unwrap();
    //         thread::sleep(Duration::from_millis(100));
    //     }
    // });

    let me: u32 = 4126515649;

    fn print_type<T>(_: &T) {
        println!("{:?}", std::any::type_name::<T>());
    }

    // Iterate to poll the eventloop for connection progress
    for (_, notification) in connection.iter().enumerate() {
        if let Ok(Event::Incoming(Incoming::Publish(publish))) = notification {
            let decoded_packet = ServiceEnvelope::decode(publish.payload);
            if let Ok(envelope) = decoded_packet {
                if let Some(meshpacket) = envelope.packet {
                    if let Some(mesh_packet::PayloadVariant::Decoded(variant)) =
                        meshpacket.payload_variant
                    {
                        if variant.portnum == PortNum::TextMessageApp as i32 {
                            let message = std::str::from_utf8(&variant.payload);
                            println!(
                                "Message {:x}=>{:x}: '{}'",
                                meshpacket.from,
                                meshpacket.to,
                                message.unwrap()
                            );
                        }
                        if variant.portnum == PortNum::NodeinfoApp as i32 {
                            let user = User::decode(&variant.payload[..]).unwrap();
                            println!("User: {}:{}/{}", user.id, user.short_name, user.long_name);
                        }
                    }
                }
            }
        }
    }
}

fn hex_node(node_num: u32) -> String {
    format!("!{:x}", node_num)
}

fn handle_from_radio_packet(
    our_id: u32,
    from_radio_packet: meshtastic::protobufs::FromRadio,
) -> Option<(String, String)> {
    match from_radio_packet.payload_variant? {
        meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
            let user = node_info.user?;
            println!("Saw {}:{}/{}", user.id, user.short_name, user.long_name);
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
