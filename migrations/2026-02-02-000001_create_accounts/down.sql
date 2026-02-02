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
