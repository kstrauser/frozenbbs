CREATE TABLE queued_messages (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at_us BIGINT UNIQUE NOT NULL,
    sent_at_us BIGINT,
    FOREIGN KEY (user_id) REFERENCES users (id))
