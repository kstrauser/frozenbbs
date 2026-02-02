-- Create accounts table for human users
CREATE TABLE accounts (
    id INTEGER NOT NULL PRIMARY KEY,
    username TEXT,
    jackass BOOL NOT NULL DEFAULT FALSE,
    bio TEXT,
    created_at_us BIGINT NOT NULL,
    last_acted_at_us BIGINT,
    in_board INTEGER,
    FOREIGN KEY (in_board) REFERENCES boards (id)
);

-- Create new nodes table from users
CREATE TABLE nodes (
    id INTEGER NOT NULL PRIMARY KEY,
    account_id INTEGER NOT NULL,
    node_id TEXT NOT NULL UNIQUE,
    short_name TEXT NOT NULL,
    long_name TEXT NOT NULL,
    created_at_us BIGINT NOT NULL,
    last_seen_at_us BIGINT NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts (id)
);

-- Migrate data: create an account for each existing user
INSERT INTO accounts (id, username, jackass, bio, created_at_us, last_acted_at_us, in_board)
SELECT id, NULL, jackass, bio, created_at_us, last_acted_at_us, in_board
FROM users;

-- Migrate users to nodes, linking to their account
INSERT INTO nodes (id, account_id, node_id, short_name, long_name, created_at_us, last_seen_at_us)
SELECT id, id, node_id, short_name, long_name, created_at_us, last_seen_at_us
FROM users;

-- Drop the old users table
DROP TABLE users;

-- Migrate posts: rename user_id to account_id, update FK to reference accounts
CREATE TABLE posts_new (
    id INTEGER NOT NULL PRIMARY KEY,
    board_id INTEGER NOT NULL,
    account_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at_us BIGINT NOT NULL,
    UNIQUE(created_at_us),
    FOREIGN KEY (account_id) REFERENCES accounts (id),
    FOREIGN KEY (board_id) REFERENCES boards (id)
);
INSERT INTO posts_new (id, board_id, account_id, body, created_at_us)
SELECT p.id, p.board_id, n.account_id, p.body, p.created_at_us
FROM posts p
JOIN nodes n ON n.id = p.user_id;
DROP TABLE posts;
ALTER TABLE posts_new RENAME TO posts;

-- Migrate board_states: rename user_id to account_id, update FK to reference accounts
CREATE TABLE board_states_new (
    id INTEGER NOT NULL PRIMARY KEY,
    account_id INTEGER NOT NULL,
    board_id INTEGER NOT NULL,
    last_post_us BIGINT NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts (id),
    FOREIGN KEY (board_id) REFERENCES boards (id),
    UNIQUE (account_id, board_id)
);
INSERT INTO board_states_new (id, account_id, board_id, last_post_us)
SELECT bs.id, n.account_id, bs.board_id, bs.last_post_us
FROM board_states bs
JOIN nodes n ON n.id = bs.user_id;
DROP TABLE board_states;
ALTER TABLE board_states_new RENAME TO board_states;

-- Migrate queued_messages: rename sender_id/recipient_id to sender_account_id/recipient_account_id
CREATE TABLE queued_messages_new (
    id INTEGER NOT NULL PRIMARY KEY,
    sender_account_id INTEGER NOT NULL,
    recipient_account_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at_us BIGINT NOT NULL,
    sent_at_us BIGINT,
    FOREIGN KEY (sender_account_id) REFERENCES accounts (id),
    FOREIGN KEY (recipient_account_id) REFERENCES accounts (id)
);
INSERT INTO queued_messages_new (id, sender_account_id, recipient_account_id, body, created_at_us, sent_at_us)
SELECT qm.id, sn.account_id, rn.account_id, qm.body, qm.created_at_us, qm.sent_at_us
FROM queued_messages qm
JOIN nodes sn ON sn.id = qm.sender_id
JOIN nodes rn ON rn.id = qm.recipient_id;
DROP TABLE queued_messages;
ALTER TABLE queued_messages_new RENAME TO queued_messages;
