use crate::{
    client::dispatch,
    commands::{self, Replies, ReplyDestination},
    db::{queued_messages, stats, users},
    hex_id_to_num, num_id_to_hex,
    paginate::{paginate, MAX_LENGTH},
    BBSConfig,
};
use diesel::SqliteConnection;
use meshtastic::{
    self,
    api::StreamApi,
    packet::{PacketDestination, PacketRouter},
    protobufs::{from_radio, mesh_packet, FromRadio, MapReport, MeshPacket, PortNum, User},
    types::NodeId,
    utils, Message,
};
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

// A simple error type
#[derive(Debug)]
pub struct TestRouterError(String);

impl Display for TestRouterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

impl Error for TestRouterError {}

// Metadata type for demonstration
pub struct HandlerMetadata {}

// Your packet router implementation
pub struct TestPacketRouter {
    pub my_id: NodeId,
}

impl PacketRouter<HandlerMetadata, TestRouterError> for TestPacketRouter {
    fn handle_packet_from_radio(
        &mut self,
        packet: FromRadio,
    ) -> Result<HandlerMetadata, TestRouterError> {
        // Check the packet
        log::debug!("handle_packet_from_radio: {packet:#?}");

        Ok(HandlerMetadata {})
    }

    fn handle_mesh_packet(
        &mut self,
        packet: MeshPacket,
    ) -> Result<HandlerMetadata, TestRouterError> {
        // Check the packet
        log::debug!("handle_mesh_packet: {packet:#?}");

        if self.my_id == packet.to {
            panic!("I got tricked into messaging myself. I'd rather panic than blue up the radio.");
        }

        Ok(HandlerMetadata {})
    }

    fn source_node_id(&self) -> NodeId {
        // Return the current node's ID
        log::debug!("My_id requested: value is {}", self.my_id);
        self.my_id
    }
}

/// Replies that commands send back to the radio.
#[derive(Debug)]
struct Response {
    sender: u32,
    replies: Option<Replies>,
}

pub async fn event_loop(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let commands = commands::command_structure(cfg);
    let stream_api = StreamApi::new();

    let connected_stream_api;
    let mut decoded_listener;

    eprintln!(
        "\
Startup stats:

{}
",
        stats(conn)
    );

    if let Some(tcp_address) = &cfg.tcp_address {
        log::info!("Connecting to {tcp_address}");
        let stream = utils::stream::build_tcp_stream(tcp_address.clone()).await?;
        (decoded_listener, connected_stream_api) = stream_api.connect(stream).await;
    } else if let Some(serial_device) = &cfg.serial_device {
        log::info!("Connecting to {serial_device}");
        let stream = utils::stream::build_serial_stream(serial_device.clone(), None, None, None)?;
        (decoded_listener, connected_stream_api) = stream_api.connect(stream).await;
    } else {
        panic!("At least one of tcp_address and serial_device must be configured.");
    }

    let config_id = utils::generate_rand_id();
    let mut stream_api = connected_stream_api.configure(config_id).await?;

    let my_id = hex_id_to_num(&cfg.my_id).unwrap();
    let mut router = TestPacketRouter {
        my_id: my_id.into(),
    };

    while let Some(decoded) = decoded_listener.recv().await {
        let Some(response) = handle_packet(conn, cfg, &commands, decoded, my_id) else {
            continue;
        };

        // Send any replies from the commands the user executed.
        if let Some(replies) = response.replies {
            for reply in replies.0 {
                let (channel, destination) = match reply.destination {
                    ReplyDestination::Sender => {
                        (0, PacketDestination::Node(NodeId::new(response.sender)))
                    }
                    ReplyDestination::Broadcast => {
                        (cfg.public_channel, PacketDestination::Broadcast)
                    }
                };
                for page in paginate(reply.out, MAX_LENGTH) {
                    stream_api
                        .send_text(&mut router, page, destination, true, channel.into())
                        .await?;
                }
            }
        }

        // Next, send any queued messages to the user.

        let node_id = num_id_to_hex(response.sender);
        let Ok(user) = users::get(conn, &node_id) else {
            // This should never happen because we should've upserted the user before calling this.
            log::debug!("No user matching {node_id}");
            continue;
        };

        let queue = queued_messages::get(conn, &user);
        if queue.is_empty() {
            log::debug!("No unsent messages for {}", user.id);
        }

        for message in queue {
            // If this becomes super popular, consider caching the user objects so we don't look
            // them up repeatedly in a loop.
            let sender = users::get_by_user_id(conn, message.sender_id);
            // This should never happen. If the sender doesn't exist, how'd this message get here?
            let Ok(sender) = sender else {
                log::error!("Unknown sender: {sender:?}");
                continue;
            };
            log::info!("Sending a queued message from {sender} to {user}");
            // Construct the message body.
            let out = vec![
                format!("Message from {} at {}:", sender, message.created_at(),),
                String::new(),
                message.body.to_string(),
            ];
            let destination = PacketDestination::Node(NodeId::new(user.node_id_numeric()));
            for page in paginate(out, MAX_LENGTH) {
                stream_api
                    .send_text(&mut router, page, destination, true, 0.into())
                    .await?;
            }
            queued_messages::sent(conn, &message);
        }
    }

    Ok(())
}

fn handle_packet(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    menus: &commands::Menus,
    radio_packet: FromRadio,
    my_id: u32,
) -> Option<Response> {
    let payload_variant = radio_packet.payload_variant?;
    let from_radio::PayloadVariant::Packet(meshpacket) = payload_variant else {
        return None;
    };
    let payload_variant = meshpacket.payload_variant?;
    let mesh_packet::PayloadVariant::Decoded(decoded) = payload_variant else {
        return None;
    };

    let mut user_id = num_id_to_hex(meshpacket.from);

    if decoded.portnum == PortNum::TextMessageApp as i32 && meshpacket.to == my_id {
        let command = match std::str::from_utf8(&decoded.payload) {
            Ok(x) => x,
            Err(err) => {
                log::error!("Unable to interpret {:?}: {err}", decoded.payload);
                return None;
            }
        };
        log::debug!("Received command from {user_id}: <{command}>");
        let replies = dispatch(conn, cfg, &user_id, menus, command.trim(), false);
        log::debug!("Result: {:?}", &replies);
        return Some(Response {
            sender: meshpacket.from,
            replies: Some(replies),
        });
    }

    let mut short_name: Option<String> = None;
    let mut long_name: Option<String> = None;

    if decoded.portnum == PortNum::MapReportApp as i32 {
        let map_report = match MapReport::decode(&decoded.payload[..]) {
            Ok(x) => x,
            Err(err) => {
                log::error!(
                    "Unable to decode the map report {:?}: {err}",
                    decoded.payload
                );
                return None;
            }
        };
        short_name = Some(map_report.short_name);
        long_name = Some(map_report.long_name);
    } else if decoded.portnum == PortNum::NodeinfoApp as i32 {
        let user = match User::decode(&decoded.payload[..]) {
            Ok(x) => x,
            Err(err) => {
                log::error!("Unable to decode the user {:?}: {err}", decoded.payload);
                return None;
            }
        };
        user_id = user.id;
        short_name = Some(user.short_name);
        long_name = Some(user.long_name);
    }
    observe(
        conn,
        &user_id,
        short_name.as_deref(),
        long_name.as_deref(),
        meshpacket.rx_time,
        decoded.portnum,
    );

    Some(Response {
        sender: meshpacket.from,
        replies: None,
    })
}

/// Wrapper around calling users::observe
fn observe(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: Option<&str>,
    long_name: Option<&str>,
    rx_time: u32,
    portnum: i32,
) {
    let label = match PortNum::try_from(portnum) {
        Ok(x) => x.as_str_name(),
        _ => &format!("portnum {portnum}"),
    };

    if let Ok((bbs_user, seen)) = users::observe(
        conn,
        node_id,
        short_name,
        long_name,
        i64::from(rx_time) * 1_000_000,
    ) {
        if seen {
            log::debug!("Observed via {label} at {rx_time}: {bbs_user}");
        } else {
            log::info!("Observed new via {label} at {rx_time}: {bbs_user}");
        }
    }
}
