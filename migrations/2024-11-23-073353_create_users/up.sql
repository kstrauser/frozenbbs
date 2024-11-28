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
  FOREIGN KEY(in_board) REFERENCES boards(id)
)
