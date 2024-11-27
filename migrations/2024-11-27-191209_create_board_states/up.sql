CREATE TABLE board_states (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  board_id INTEGER NOT NULL,
  last_post_us BIGINT NOT NULL,
  FOREIGN KEY(user_id) REFERENCES users(id),
  FOREIGN KEY(board_id) REFERENCES boards(id),
  UNIQUE(user_id, board_id)
)
