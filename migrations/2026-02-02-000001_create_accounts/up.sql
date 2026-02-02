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

-- Create new nodes table from users (renaming)
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
