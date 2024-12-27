CREATE TABLE queued_messages (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    sender_id INTEGER NOT NULL,
    recipient_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at_us BIGINT UNIQUE NOT NULL,
    sent_at_us BIGINT,
    FOREIGN KEY (sender_id) REFERENCES users (id),
    FOREIGN KEY (recipient_id) REFERENCES users (id))
