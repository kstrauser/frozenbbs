use super::models::{QueuedMessage, User};
use super::now_as_useconds;
use super::schema::queued_messages::{dsl, table};
use diesel::prelude::*;

/// Get any queued messages for this user.
pub fn get(conn: &mut SqliteConnection, user: &User) -> Vec<QueuedMessage> {
    table
        .select(QueuedMessage::as_select())
        .filter(dsl::recipient_id.eq(user.id))
        .filter(dsl::sent_at_us.is_null())
        .load(conn)
        .expect("should always be possible to get queued messages")
}

/// Mark a message as sent.
pub fn sent(conn: &mut SqliteConnection, message: &QueuedMessage) {
    let now = now_as_useconds();
    diesel::update(&message)
        .set(dsl::sent_at_us.eq(now))
        .execute(conn)
        .expect("should always be possible to mark a queued message as sent");
}
