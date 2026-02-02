use super::formatted_useconds;
use super::schema::{accounts, board_states, boards, nodes, posts};
use crate::hex_id_to_num;
use diesel::prelude::*;
use regex::Regex;
use std::fmt;
use validator::Validate;

use once_cell::sync::Lazy;

static RE_NODE_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^![0-9a-f]{8}$").unwrap());
// This seems like a reasonable range to clamp timestamps to. Because we're dealing with
// microseconds, it's good to enforce a plausible range so that things will blow up if we
// inadvertently try to use seconds, milliseconds, or nanoseconds somewhere.
const EARLY_2024: i64 = 1_704_096_000_000_000;
const EARLY_2200: i64 = 7_258_147_200_000_000;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::boards)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Board {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub created_at_us: i64,
}

impl Board {
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} {}: {}", self.id, self.name, self.description)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = boards)]
pub struct NewBoard<'a> {
    #[validate(length(min = 1, max = 30))]
    pub name: &'a str,
    #[validate(length(min = 1, max = 100))]
    pub description: &'a str,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(belongs_to(Board))]
#[diesel(table_name = crate::db::schema::posts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Post {
    pub id: i32,
    pub board_id: i32,
    pub user_id: i32,
    pub body: String,
    pub created_at_us: i64,
}

impl Post {
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
    #[validate(range(min = 1))]
    pub user_id: i32,
    #[validate(range(min = 1))]
    pub board_id: i32,
    #[validate(length(min = 1))]
    pub body: &'a str,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
}

/// An account represents a human (or machine) user of the BBS.
/// Accounts can have one or more nodes associated with them.
#[derive(Debug, Identifiable, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::db::schema::accounts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Account {
    pub id: i32,
    pub username: Option<String>,
    pub jackass: bool,
    pub bio: Option<String>,
    pub created_at_us: i64,
    pub last_acted_at_us: Option<i64>,
    pub in_board: Option<i32>,
}

impl Account {
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
    pub fn last_acted_at(&self) -> String {
        if let Some(acted) = self.last_acted_at_us {
            formatted_useconds(acted)
        } else {
            String::new()
        }
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = accounts)]
pub struct AccountNew<'a> {
    #[validate(length(max = 40))]
    pub username: Option<&'a str>,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_acted_at_us: Option<&'a i64>,
}

#[derive(AsChangeset, Validate)]
#[diesel(table_name = accounts)]
pub struct AccountUpdate<'a> {
    #[validate(length(max = 40))]
    pub username: Option<&'a str>,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_acted_at_us: Option<&'a i64>,
    #[validate(length(min = 0))]
    pub bio: Option<String>,
}

/// A node represents a Meshtastic radio device.
/// Multiple nodes can belong to the same account.
#[derive(Debug, Identifiable, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::db::schema::nodes)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Node {
    pub id: i32,
    pub account_id: i32,
    pub node_id: String,
    pub short_name: String,
    pub long_name: String,
    pub created_at_us: i64,
    pub last_seen_at_us: i64,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}:{}", self.node_id, self.short_name, self.long_name)
    }
}

impl Node {
    pub fn node_id_numeric(&self) -> u32 {
        hex_id_to_num(&self.node_id).expect("node_ids in the database should always be valid")
    }
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
    pub fn last_seen_at(&self) -> String {
        formatted_useconds(self.last_seen_at_us)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = nodes)]
pub struct NodeNew<'a> {
    #[validate(range(min = 1))]
    pub account_id: i32,
    #[validate(regex(path = *RE_NODE_ID))]
    pub node_id: &'a str,
    #[validate(length(min = 1, max = 4))]
    pub short_name: &'a str,
    #[validate(length(min = 1, max = 40))]
    pub long_name: &'a str,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_seen_at_us: &'a i64,
}

#[derive(AsChangeset, Validate)]
#[diesel(table_name = nodes)]
pub struct NodeUpdate<'a> {
    #[validate(length(min = 1, max = 4))]
    pub short_name: Option<&'a str>,
    #[validate(length(min = 1, max = 40))]
    pub long_name: Option<&'a str>,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_seen_at_us: Option<&'a i64>,
}

/// A combined view of an Account and its primary Node.
/// This is used throughout the application where we need both account and node info.
#[derive(Debug, Clone)]
pub struct User {
    pub account: Account,
    pub node: Node,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(username) = &self.account.username {
            write!(f, "{}", username)
        } else {
            write!(f, "{}", self.node.long_name)
        }
    }
}

impl User {
    pub fn node_id_numeric(&self) -> u32 {
        self.node.node_id_numeric()
    }
    pub fn created_at(&self) -> String {
        self.account.created_at()
    }
    pub fn last_acted_at(&self) -> String {
        self.account.last_acted_at()
    }
    pub fn last_seen_at(&self) -> String {
        self.node.last_seen_at()
    }
    pub fn jackass(&self) -> bool {
        self.account.jackass
    }
    pub fn bio(&self) -> &Option<String> {
        &self.account.bio
    }
    pub fn in_board(&self) -> Option<i32> {
        self.account.in_board
    }
    pub fn account_id(&self) -> i32 {
        self.account.id
    }
    pub fn node_id(&self) -> &str {
        &self.node.node_id
    }
    pub fn short_name(&self) -> &str {
        &self.node.short_name
    }
    pub fn long_name(&self) -> &str {
        &self.node.long_name
    }
    /// Returns the display name: username if set, otherwise long_name from node
    pub fn display_name(&self) -> &str {
        self.account.username.as_deref().unwrap_or(&self.node.long_name)
    }
}

#[derive(Debug, Identifiable, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::board_states)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct BoardState {
    pub id: i32,
    pub user_id: i32,
    pub board_id: i32,
    pub last_post_us: i64,
}

#[derive(Debug, Insertable, Validate)]
#[diesel(table_name = board_states)]
pub struct NewBoardState {
    #[validate(range(min = 1))]
    pub user_id: i32,
    #[validate(range(min = 1))]
    pub board_id: i32,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub last_post_us: i64,
}

#[derive(Debug, Identifiable, Queryable, Selectable)]
#[diesel(table_name = crate::db::schema::queued_messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct QueuedMessage {
    pub id: i32,
    pub sender_id: i32,
    pub recipient_id: i32,
    pub body: String,
    pub created_at_us: i64,
    pub sent_at_us: Option<i64>,
}

impl QueuedMessage {
    pub fn created_at(&self) -> String {
        formatted_useconds(self.created_at_us)
    }
}

#[derive(Insertable, Validate)]
#[diesel(table_name = crate::db::schema::queued_messages)]
pub struct QueuedMessageNew<'a> {
    #[validate(range(min = 1))]
    pub sender_id: i32,
    #[validate(range(min = 1))]
    pub recipient_id: i32,
    #[validate(length(min = 1))]
    pub body: &'a str,
    #[validate(range(min = EARLY_2024, max=EARLY_2200))]
    pub created_at_us: &'a i64,
}
