use super::{Replies, Reply, ReplyDestination};
use crate::db::User;
use crate::{system_info, BBSConfig};
use diesel::SqliteConnection;

/// Send a BBS advertisement to the main channel.
pub fn advertise(
    _conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    Replies(vec![
        Reply {
            out: vec![cfg.ad_text.clone(), String::new(), system_info(cfg)],
            destination: ReplyDestination::Broadcast,
        },
        Reply {
            out: vec!["You have spammed the broadcast channel.".to_string()],
            destination: ReplyDestination::Sender,
        },
    ])
}
