-- Drop invitations table
DROP TABLE invitations;

-- SQLite doesn't support DROP COLUMN before 3.35.0, so recreate accounts without invite_allowed
CREATE TABLE accounts_new (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    username TEXT,
    jackass BOOL NOT NULL DEFAULT FALSE,
    bio TEXT,
    created_at_us BIGINT NOT NULL,
    last_acted_at_us BIGINT,
    in_board INTEGER,
    FOREIGN KEY (in_board) REFERENCES boards (id)
);
INSERT INTO accounts_new (id, username, jackass, bio, created_at_us, last_acted_at_us, in_board)
SELECT id, username, jackass, bio, created_at_us, last_acted_at_us, in_board FROM accounts;
DROP TABLE accounts;
ALTER TABLE accounts_new RENAME TO accounts;
