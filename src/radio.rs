/// This example connects to a TCP port on the radio, and prints out all received packets.
/// This can be used with a simulated radio via the Meshtastic Docker firmware image.
/// https://meshtastic.org/docs/software/linux-native#usage-with-docker
use diesel::SqliteConnection;
extern crate meshtastic;
use crate::client::dispatch;
use crate::commands::setup;
use crate::node_id_from_hex;
use meshtastic::api::StreamApi;
use meshtastic::utils;

// This import allows for decoding of mesh packets
// Re-export of prost::Message
// use meshtastic::Message;

use std::io::{self, BufRead};
use std::time::SystemTime;

// /// Set up the logger to output to stdout
// /// **Note:** the invokation of this function is commented out in main by default.
// fn setup_logger() -> Result<(), fern::InitError> {
//     fern::Dispatch::new()
//         .format(|out, message, record| {
//             out.finish(format_args!(
//                 "[{} {} {}] {}",
//                 humantime::format_rfc3339_seconds(SystemTime::now()),
//                 record.level(),
//                 record.target(),
//                 message
//             ))
//         })
//         .level(log::LevelFilter::Trace)
//         .chain(std::io::stdout())
//         .apply()?;
//
//     Ok(())
// }

fn bullshit_send(recipient: &str, message: &str) {
    use std::process::Command;

    let b = Command::new("./meshtastic-python")
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
    dbg!(b);
    println!("Sent");
}

pub async fn event_loop(
    conn: &mut SqliteConnection,
    our_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Uncomment this to enable logging
    // setup_logger()?;

    let stream_api = StreamApi::new();
    let our_id = node_id_from_hex(our_id);

    // println!("Enter the address of a TCP port to connect to, in the form \"IP:PORT\":");

    // let stdin = io::stdin();
    // let entered_address = stdin
    //     .lock()
    //     .lines()
    //     .next()
    //     .expect("Failed to find next line")
    //     .expect("Could not read next line");

    let entered_address = "localhost:4403".to_string();

    let tcp_stream = utils::stream::build_tcp_stream(entered_address).await?;
    let (mut decoded_listener, stream_api) = stream_api.connect(tcp_stream).await;

    let config_id = utils::generate_rand_id();
    let stream_api = stream_api.configure(config_id).await?;

    use meshtastic::utils::generate_rand_id;
    use meshtastic::utils::stream::build_tcp_stream;
    let stream_api = StreamApi::new();
    let tcp_stream = build_tcp_stream("localhost:4403".to_string()).await?;
    let (_decoded_listener, stream_api) = stream_api.connect(tcp_stream).await;
    let config_id = generate_rand_id();
    let stream_api = stream_api.configure(config_id).await?;

    let commands = setup();

    // This loop can be broken with ctrl+c, or by unpowering the radio.
    while let Some(decoded) = decoded_listener.recv().await {
        // println!("Received: {:?}", decoded);
        if let Some((node_id, command)) = handle_from_radio_packet(conn, our_id, decoded) {
            println!("Received command from {}: <{}>", node_id, command);
            let result = dispatch(conn, &node_id, &commands, command.trim());
            print!("{}", &result);
            bullshit_send(&node_id, &result.trim());
            println!("Back in the loop");
            break;
        }
    }

    // Note that in this specific example, this will only be called when
    // the radio is disconnected, as the above loop will never exit.
    // Typically you would allow the user to manually kill the loop,
    // for example with tokio::select!.
    let _stream_api = stream_api.disconnect().await?;

    Ok(())
}

fn hex_node(node_num: u32) -> String {
    format!("!{:x}", node_num)
}

/// A helper function to handle packets coming directly from the radio connection.
/// The Meshtastic `PhoneAPI` will return decoded `FromRadio` packets, which
/// can then be handled based on their payload variant. Note that the payload
/// variant can be `None`, in which case the packet should be ignored.
fn handle_from_radio_packet(
    conn: &mut SqliteConnection,
    our_id: u32,
    from_radio_packet: meshtastic::protobufs::FromRadio,
) -> Option<(String, String)> {
    // Remove `None` variants to get the payload variant
    let payload_variant = match from_radio_packet.payload_variant {
        Some(payload_variant) => payload_variant,
        None => {
            // println!("Received FromRadio packet with no payload variant, not handling...");
            return None;
        }
    };

    // `FromRadio` packets can be differentiated based on their payload variant,
    // which in Rust is represented as an enum. This means the payload variant
    // can be matched on, and the appropriate user-defined action can be taken.
    match payload_variant {
        meshtastic::protobufs::from_radio::PayloadVariant::Channel(channel) => {
            // println!("Received channel packet: {:?}", channel);
        }
        meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
            if let Some(user) = node_info.user {
                use crate::db::users;

                println!(
                    "Heard node at {}: {}:{}/{}",
                    node_info.last_heard, user.id, user.short_name, user.long_name
                );

                let u = users::observe(
                    conn,
                    &user.id,
                    &user.short_name,
                    &user.long_name,
                    node_info.last_heard as i64 * 1_000_000,
                );
                dbg!(&u);
            }
        }
        meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet) => {
            // println!("Received mesh pack: {:?}", mesh_packet);
            if let Some((node_id, command)) = handle_mesh_packet(mesh_packet, our_id) {
                return Some((node_id, command));
            }
        }
        _ => {
            // println!("Received other FromRadio packet, not handling...");
        }
    };
    None
}

/// A helper function to handle `MeshPacket` messages, which are a subset
/// of all `FromRadio` messages. Note that the payload variant can be `None`,
/// and that the payload variant can be `Encrypted`, in which case the packet
/// should be ignored within client applications.
///
/// Mesh packets are the most commonly used type of packet, and are usually
/// what people are referring to when they talk about "packets."
fn handle_mesh_packet(
    mesh_packet: meshtastic::protobufs::MeshPacket,
    our_id: u32,
) -> Option<(String, String)> {
    // Remove `None` variants to get the payload variant
    let payload_variant = match &mesh_packet.payload_variant {
        Some(payload_variant) => payload_variant,
        None => {
            // println!("Received mesh packet with no payload variant, not handling...");
            return None;
        }
    };

    // Only handle decoded (unencrypted) mesh packets
    let packet_data = match payload_variant {
        meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(decoded_mesh_packet) => {
            decoded_mesh_packet
        }
        meshtastic::protobufs::mesh_packet::PayloadVariant::Encrypted(_encrypted_mesh_packet) => {
            // println!("Received encrypted mesh packet, not handling...");
            return None;
        }
    };

    // Meshtastic differentiates mesh packets based on a field called `portnum`.
    // Meshtastic defines a set of standard port numbers [here](https://meshtastic.org/docs/development/firmware/portnum),
    // but also allows for custom port numbers to be used.
    match packet_data.portnum() {
        //         meshtastic::protobufs::PortNum::PositionApp => {
        //             // Note that `Data` structs contain a `payload` field, which is a vector of bytes.
        //             // This data needs to be decoded into a protobuf struct, which is shown below.
        //             // The `decode` function is provided by the `prost` crate, which is re-exported
        //             // by the `meshtastic` crate.
        //             let decoded_position =
        //                 meshtastic::protobufs::Position::decode(packet_data.payload.as_slice()).unwrap();
        //
        //             println!("Received position packet: {:?}", decoded_position);
        //         }
        meshtastic::protobufs::PortNum::TextMessageApp => {
            let decoded_text_message = String::from_utf8(packet_data.payload.clone()).unwrap();

            // println!("USER: {:?}", &mesh_packet);
            // println!("USER: {:?}", &packet_data);
            // DMs: to == radio's own ID, channel = 0
            // Public: to == 0xffffffff
            eprintln!(
                "USER: Received text message packet from {:x} to {:x} in channel {}: {}",
                mesh_packet.from, mesh_packet.to, mesh_packet.channel, decoded_text_message
            );
            if mesh_packet.to == our_id {
                return Some((hex_node(mesh_packet.from), decoded_text_message));
            }
            None
        }
        //         meshtastic::protobufs::PortNum::WaypointApp => {
        //             let decoded_waypoint =
        //                 meshtastic::protobufs::Waypoint::decode(packet_data.payload.as_slice()).unwrap();
        //
        //             println!("Received waypoint packet: {:?}", decoded_waypoint);
        //         }
        _ => {
            // println!(
            //     "Received mesh packet on port {:?}, not handling...",
            //     packet_data.portnum
            // );
            None
        }
    }
}
