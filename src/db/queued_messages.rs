use super::models::{QueuedMessage, QueuedMessageNew, User};
use super::schema::queued_messages::{dsl, table};
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

/// Get any queued messages for this user.
pub fn get(conn: &mut SqliteConnection, user: &User) -> Vec<QueuedMessage> {
    table
        .select(QueuedMessage::as_select())
        .filter(dsl::recipient_account_id.eq(user.account_id()))
        .filter(dsl::sent_at_us.is_null())
        .load(conn)
        .expect("should always be possible to get queued messages")
}

/// Queue a message for a user.
pub fn post(
    conn: &mut SqliteConnection,
    sender: &User,
    recipient: &User,
    body: &str,
) -> Result<QueuedMessage> {
    let new_post = QueuedMessageNew {
        sender_account_id: sender.account_id(),
        recipient_account_id: recipient.account_id(),
        body,
        created_at_us: &now_as_useconds(),
    };
    new_post.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_post)
        .returning(QueuedMessage::as_returning())
        .get_result(conn)
        .expect("should always be able to insert a new post"))
}

/// Mark a message as sent.
pub fn sent(conn: &mut SqliteConnection, message: &QueuedMessage) {
    let now = now_as_useconds();
    diesel::update(&message)
        .set(dsl::sent_at_us.eq(now))
        .execute(conn)
        .expect("should always be possible to mark a queued message as sent");
}
