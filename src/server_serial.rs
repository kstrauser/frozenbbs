use crate::hex_id_to_num;
use crate::paginate::{paginate, MAX_LENGTH};
use crate::{client::dispatch, commands, db::users, num_id_to_hex, BBSConfig};
use diesel::SqliteConnection;
use meshtastic;
use meshtastic::api::StreamApi;
use meshtastic::packet::PacketDestination;
use meshtastic::packet::PacketRouter;
use meshtastic::protobufs::{from_radio, mesh_packet, PortNum, User};
use meshtastic::protobufs::{FromRadio, MeshPacket};
use meshtastic::types::NodeId;
use meshtastic::utils;
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

    eprintln!("Listening for messages.");

    while let Some(decoded) = decoded_listener.recv().await {
        if let Some((sender, out)) = handle_packet(conn, &commands, decoded, my_id) {
            for page in paginate(out, MAX_LENGTH) {
                use meshtastic::types::NodeId;
                stream_api
                    .send_text(
                        &mut router,
                        page,
                        PacketDestination::Node(NodeId::new(sender)),
                        true,
                        0.into(),
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

fn handle_packet(
    conn: &mut SqliteConnection,
    commands: &Vec<commands::Command>,
    radio_packet: FromRadio,
    my_id: u32,
) -> Option<(u32, Vec<String>)> {
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
        let out = dispatch(conn, &node_id, commands, command.trim());
        log::debug!("Result: {:?}", &out);
        return Some((meshpacket.from, out));
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
