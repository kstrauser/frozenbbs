use crate::{
    client::dispatch,
    commands::{self, Reply, ReplyDestination},
    db::{stats, users},
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
        log::debug!("{:#?}", packet);

        Ok(HandlerMetadata {})
    }

    fn handle_mesh_packet(
        &mut self,
        packet: MeshPacket,
    ) -> Result<HandlerMetadata, TestRouterError> {
        // Check the packet
        log::debug!("{:#?}", packet);

        Ok(HandlerMetadata {})
    }

    fn source_node_id(&self) -> NodeId {
        // Return the current node's ID
        log::debug!("My_id requested: value is {}", self.my_id);
        self.my_id
    }
}

/// Replies that commands send back to the radio.
struct Response {
    sender: u32,
    reply: Reply,
}

pub async fn event_loop(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let commands = commands::command_structure();
    let stream_api = StreamApi::new();

    let connected_stream_api;
    let mut decoded_listener;

    if let Some(tcp_address) = &cfg.tcp_address {
        log::info!("Connecting to {tcp_address}");
        let stream = utils::stream::build_tcp_stream(tcp_address.clone()).await?;
        (decoded_listener, connected_stream_api) = stream_api.connect(stream).await;
    } else if let Some(serial_device) = &cfg.serial_device {
        log::info!("Connecting to {serial_device}");
        let stream = utils::stream::build_serial_stream(serial_device.clone(), None, None, None)?;
        (decoded_listener, connected_stream_api) = stream_api.connect(stream).await;
    } else {
        panic!("Exactly one of tcp_address and serial_device must be configured.");
    }

    let config_id = utils::generate_rand_id();
    let mut stream_api = connected_stream_api.configure(config_id).await?;

    let my_id = hex_id_to_num(&cfg.my_id);
    let mut router = TestPacketRouter {
        my_id: my_id.into(),
    };

    eprintln!(
        "\
Startup stats:

{}

Listening for messages.",
        stats(conn)
    );

    while let Some(decoded) = decoded_listener.recv().await {
        if let Some(response) = handle_packet(conn, cfg, &commands, decoded, my_id) {
            let (channel, destination) = match response.reply.destination {
                ReplyDestination::Sender => {
                    (0, PacketDestination::Node(NodeId::new(response.sender)))
                }
                ReplyDestination::Broadcast => (cfg.public_channel, PacketDestination::Broadcast),
            };
            for page in paginate(response.reply.out, MAX_LENGTH) {
                stream_api
                    .send_text(&mut router, page, destination, true, channel.into())
                    .await?;
            }
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

    if decoded.portnum == PortNum::TextMessageApp as i32 && meshpacket.to == my_id {
        let node_id = num_id_to_hex(meshpacket.from);
        let command = match std::str::from_utf8(&decoded.payload) {
            Ok(x) => x,
            Err(err) => {
                log::error!("Unable to interpret {:?}: {err}", decoded.payload);
                return None;
            }
        };
        log::debug!("Received command from {}: <{}>", node_id, command);
        let reply = dispatch(conn, cfg, &node_id, menus, command.trim());
        log::debug!("Result: {:?}", &reply.out);
        return Some(Response {
            sender: meshpacket.from,
            reply,
        });
    }

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
        if let Ok((bbs_user, seen)) = users::observe(
            conn,
            &num_id_to_hex(meshpacket.from),
            Some(&map_report.short_name),
            Some(&map_report.long_name),
            i64::from(meshpacket.rx_time) * 1_000_000,
        ) {
            if seen {
                log::info!("Observed MapReport at {}: {}", meshpacket.rx_time, bbs_user);
            } else {
                log::info!(
                    "Observed MapReport new at {}: {}",
                    meshpacket.rx_time,
                    bbs_user
                );
            }
        };
    }

    if decoded.portnum == PortNum::NodeinfoApp as i32 {
        let user = match User::decode(&decoded.payload[..]) {
            Ok(x) => x,
            Err(err) => {
                log::error!("Unable to decode the user {:?}: {err}", decoded.payload);
                return None;
            }
        };
        if let Ok((bbs_user, seen)) = users::observe(
            conn,
            &user.id,
            Some(&user.short_name),
            Some(&user.long_name),
            i64::from(meshpacket.rx_time) * 1_000_000,
        ) {
            if seen {
                log::info!("Observed NodeInfo at {}: {}", meshpacket.rx_time, bbs_user);
            } else {
                log::info!(
                    "Observed NodeInfo new at {}: {}",
                    meshpacket.rx_time,
                    bbs_user
                );
            }
        };
    }
    None
}
