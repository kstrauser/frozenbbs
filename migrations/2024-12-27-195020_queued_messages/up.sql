CREATE TABLE queued_messages (
    id integer NOT NULL PRIMARY KEY AUTOINCREMENT,
    user_id integer NOT NULL,
    body text NOT NULL,
    created_at_us bigint UNIQUE NOT NULL,
    sent_at_us bigint,
    FOREIGN KEY (user_id) REFERENCES users (id),
)
