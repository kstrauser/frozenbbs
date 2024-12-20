use crate::{
    client::dispatch,
    commands,
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
    protobufs::{from_radio, mesh_packet, FromRadio, MeshPacket, PortNum, User},
    types::{MeshChannel, NodeId},
    utils,
};
use prost::Message;
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
struct ReplyMessage {
    channel: MeshChannel,
    destination: PacketDestination,
    out: Vec<String>,
}

pub async fn event_loop(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let commands = commands::setup();
    let stream_api = StreamApi::new();

    let serial_stream =
        utils::stream::build_serial_stream(cfg.serial_device.clone(), None, None, None)?;
    let (mut decoded_listener, stream_api) = stream_api.connect(serial_stream).await;

    let config_id = utils::generate_rand_id();
    let mut stream_api = stream_api.configure(config_id).await?;

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
        if let Some(reply) = handle_packet(conn, cfg, &commands, decoded, my_id) {
            for page in paginate(reply.out, MAX_LENGTH) {
                stream_api
                    .send_text(&mut router, page, reply.destination, true, reply.channel)
                    .await?;
            }
        }
    }

    Ok(())
}

fn handle_packet(
    conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    commands: &Vec<commands::Command>,
    radio_packet: FromRadio,
    my_id: u32,
) -> Option<ReplyMessage> {
    let payload_variant = match radio_packet.payload_variant {
        Some(x) => x,
        _ => return None,
    };
    let meshpacket = match payload_variant {
        from_radio::PayloadVariant::Packet(x) => x,
        _ => return None,
    };
    let payload_variant = match meshpacket.payload_variant {
        Some(x) => x,
        _ => return None,
    };
    let decoded = match payload_variant {
        mesh_packet::PayloadVariant::Decoded(x) => x,
        _ => return None,
    };
    if decoded.portnum == PortNum::TextMessageApp as i32 && meshpacket.to == my_id {
        let node_id = num_id_to_hex(meshpacket.from);
        let message = std::str::from_utf8(&decoded.payload);
        let command = message.unwrap();
        log::debug!("Received command from {}: <{}>", node_id, command);
        let out = dispatch(conn, cfg, &node_id, commands, command.trim());
        log::debug!("Result: {:?}", &out);
        return Some(ReplyMessage {
            channel: 0.into(),
            destination: PacketDestination::Node(NodeId::new(meshpacket.from)),
            out,
        });
    }
    if decoded.portnum == PortNum::NodeinfoApp as i32 {
        let user = User::decode(&decoded.payload[..]).unwrap();
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
    None
}
