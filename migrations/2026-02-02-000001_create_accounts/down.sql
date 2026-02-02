-- Restore queued_messages with original column names
CREATE TABLE queued_messages_new (
    id INTEGER NOT NULL PRIMARY KEY,
    sender_id INTEGER NOT NULL,
    recipient_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at_us BIGINT NOT NULL,
    sent_at_us BIGINT,
    FOREIGN KEY (sender_id) REFERENCES users (id),
    FOREIGN KEY (recipient_id) REFERENCES users (id)
);
INSERT INTO queued_messages_new (id, sender_id, recipient_id, body, created_at_us, sent_at_us)
SELECT qm.id, n.id, rn.id, qm.body, qm.created_at_us, qm.sent_at_us
FROM queued_messages qm
JOIN nodes n ON n.account_id = qm.sender_account_id
JOIN nodes rn ON rn.account_id = qm.recipient_account_id;
DROP TABLE queued_messages;
ALTER TABLE queued_messages_new RENAME TO queued_messages;

-- Restore board_states with original column names
CREATE TABLE board_states_new (
    id INTEGER NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    board_id INTEGER NOT NULL,
    last_post_us BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id),
    FOREIGN KEY (board_id) REFERENCES boards (id),
    UNIQUE (user_id, board_id)
);
INSERT INTO board_states_new (id, user_id, board_id, last_post_us)
SELECT bs.id, n.id, bs.board_id, bs.last_post_us
FROM board_states bs
JOIN nodes n ON n.account_id = bs.account_id;
DROP TABLE board_states;
ALTER TABLE board_states_new RENAME TO board_states;

-- Restore posts with original column names
CREATE TABLE posts_new (
    id INTEGER NOT NULL PRIMARY KEY,
    board_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at_us BIGINT NOT NULL,
    UNIQUE(created_at_us),
    FOREIGN KEY (user_id) REFERENCES users (id),
    FOREIGN KEY (board_id) REFERENCES boards (id)
);
INSERT INTO posts_new (id, board_id, user_id, body, created_at_us)
SELECT p.id, p.board_id, n.id, p.body, p.created_at_us
FROM posts p
JOIN nodes n ON n.account_id = p.account_id;
DROP TABLE posts;
ALTER TABLE posts_new RENAME TO posts;

-- Recreate the original users table
CREATE TABLE users (
    id INTEGER NOT NULL PRIMARY KEY,
    node_id TEXT NOT NULL UNIQUE,
    short_name TEXT NOT NULL,
    long_name TEXT NOT NULL,
    jackass BOOL NOT NULL DEFAULT FALSE,
    in_board INTEGER,
    created_at_us BIGINT NOT NULL,
    last_seen_at_us BIGINT NOT NULL,
    last_acted_at_us BIGINT,
    bio TEXT,
    FOREIGN KEY (in_board) REFERENCES boards (id)
);

-- Migrate data back from nodes and accounts
INSERT INTO users (id, node_id, short_name, long_name, jackass, in_board, created_at_us, last_seen_at_us, last_acted_at_us, bio)
SELECT n.id, n.node_id, n.short_name, n.long_name, a.jackass, a.in_board, n.created_at_us, n.last_seen_at_us, a.last_acted_at_us, a.bio
FROM nodes n
JOIN accounts a ON n.account_id = a.id;

-- Drop the new tables
DROP TABLE nodes;
DROP TABLE accounts;
