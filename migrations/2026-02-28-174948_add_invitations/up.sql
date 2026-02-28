-- Add invite_allowed column to accounts table
ALTER TABLE accounts ADD COLUMN invite_allowed BOOL NOT NULL DEFAULT FALSE;

-- Create invitations table
CREATE TABLE invitations (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    sender_account_id INTEGER NOT NULL,
    invitee_node_id INTEGER NOT NULL,
    password TEXT NOT NULL,
    created_at_us BIGINT NOT NULL,
    accepted_at_us BIGINT,
    denied_at_us BIGINT,
    FOREIGN KEY (sender_account_id) REFERENCES accounts (id),
    FOREIGN KEY (invitee_node_id) REFERENCES nodes (id)
);
