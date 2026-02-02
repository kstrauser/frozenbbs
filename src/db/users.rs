use super::models::{Account, AccountNew, AccountUpdate, Node, NodeNew, NodeUpdate, User};
use super::schema::accounts::{self, dsl as accounts_dsl};
use super::schema::nodes::{self, dsl as nodes_dsl};
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

const UNKNOWN: &str = "????";

// Returned the trimmed inside of an option value if it's given.
fn option_trimmed(value: Option<&str>) -> Option<&str> {
    match value {
        Some(x) => Some(x.trim()),
        None => None,
    }
}

/// Get a Node by its node_id, if it exists
fn get_node(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<Node> {
    nodes_dsl::nodes
        .select(Node::as_select())
        .filter(nodes_dsl::node_id.eq(node_id))
        .first(conn)
}

/// Get an Account by its id
fn get_account_by_id(conn: &mut SqliteConnection, account_id: i32) -> QueryResult<Account> {
    accounts_dsl::accounts
        .select(Account::as_select())
        .filter(accounts_dsl::id.eq(account_id))
        .first(conn)
}

/// Combine a Node and Account into a User
fn make_user(conn: &mut SqliteConnection, node: Node) -> User {
    let account = get_account_by_id(conn, node.account_id)
        .expect("node should always have a valid account");
    User { account, node }
}

/// Record a user that we passively saw in a `NodeInfo` packet.
///
/// Updates their `last_seen` timestamps. Returns the user object, and whether they were already in
/// the database.
///
/// I'm resisting the urge to refactor this and `record` to use the same underlying code. They're
/// just different enough that the code to handle both cases would be more complex than having 2
/// similar functions.
pub fn observe(
    conn: &mut SqliteConnection,
    node_id: &str,
    short_name: Option<&str>,
    long_name: Option<&str>,
    last_seen_at_us: i64,
) -> Result<(User, bool)> {
    // Don't accept timestamps in the future.
    let now = now_as_useconds();
    let timestamp = if last_seen_at_us > 0 {
        last_seen_at_us.min(now)
    } else {
        now
    };

    let short_name = option_trimmed(short_name);
    let long_name = option_trimmed(long_name);

    let node_update = NodeUpdate {
        short_name,
        long_name,
        last_seen_at_us: Some(&now),
    };
    node_update.validate()?;

    Ok(conn
        .transaction::<_, diesel::result::Error, _>(|conn| {
            if let Ok(node) = get_node(conn, node_id) {
                // The node already existed. Update their short name, long name, and last seen
                // timestamps.
                let updated_node: Node = diesel::update(&node)
                    .set(&node_update)
                    .returning(Node::as_returning())
                    .get_result(conn)
                    .expect("Error observing existing node");
                Ok((make_user(conn, updated_node), true))
            } else {
                // The node is new. Create an account and insert the node.
                let new_account = AccountNew {
                    username: None,
                    created_at_us: &timestamp,
                    last_acted_at_us: None,
                };
                new_account.validate().expect("new account should be valid");

                let account: Account = diesel::insert_into(accounts::table)
                    .values(&new_account)
                    .returning(Account::as_returning())
                    .get_result(conn)
                    .expect("Error creating new account");

                let new_node = NodeNew {
                    account_id: account.id,
                    node_id: node_id.trim(),
                    short_name: short_name.unwrap_or(UNKNOWN),
                    long_name: long_name.unwrap_or(UNKNOWN),
                    created_at_us: &timestamp,
                    last_seen_at_us: &timestamp,
                };
                new_node.validate().expect("new node should be valid");

                let node: Node = diesel::insert_into(nodes::table)
                    .values(&new_node)
                    .returning(Node::as_returning())
                    .get_result(conn)
                    .expect("Error creating new node");

                Ok((User { account, node }, false))
            }
        })
        .expect("we must be able to commit database transactions"))
}

/// Get information about the user executing a command.
///
/// Updates their `last_acted` and `last_seen` timestamps. Returns the user object, and whether
/// they had already interacted with the BBS.
pub fn record(conn: &mut SqliteConnection, node_id: &str) -> Result<(User, bool)> {
    let now = now_as_useconds();

    let node_update = NodeUpdate {
        short_name: None,
        long_name: None,
        last_seen_at_us: Some(&now),
    };
    node_update.validate()?;

    let account_update = AccountUpdate {
        username: None,
        last_acted_at_us: Some(&now),
        bio: None,
    };
    account_update.validate()?;

    Ok(conn
        .transaction::<_, diesel::result::Error, _>(|conn| {
            if let Ok(node) = get_node(conn, node_id) {
                // The node already existed. Update timestamps.
                let account = get_account_by_id(conn, node.account_id)
                    .expect("node should have valid account");
                let has_acted = account.last_acted_at_us.is_some();

                let updated_node: Node = diesel::update(&node)
                    .set(&node_update)
                    .returning(Node::as_returning())
                    .get_result(conn)
                    .expect("should be able to update a node");

                let updated_account: Account = diesel::update(&account)
                    .set(&account_update)
                    .returning(Account::as_returning())
                    .get_result(conn)
                    .expect("should be able to update an account");

                Ok((User { account: updated_account, node: updated_node }, has_acted))
            } else {
                // The node is new. Create an account and insert the node.
                let new_account = AccountNew {
                    username: None,
                    created_at_us: &now,
                    last_acted_at_us: Some(&now),
                };
                new_account.validate().expect("new account should be valid");

                let account: Account = diesel::insert_into(accounts::table)
                    .values(&new_account)
                    .returning(Account::as_returning())
                    .get_result(conn)
                    .expect("Error creating new account");

                let new_node = NodeNew {
                    account_id: account.id,
                    node_id: node_id.trim(),
                    short_name: UNKNOWN,
                    long_name: UNKNOWN,
                    created_at_us: &now,
                    last_seen_at_us: &now,
                };
                new_node.validate().expect("new node should be valid");

                let node: Node = diesel::insert_into(nodes::table)
                    .values(&new_node)
                    .returning(Node::as_returning())
                    .get_result(conn)
                    .expect("Error creating new node");

                Ok((User { account, node }, false))
            }
        })
        .expect("we must be able to commit database transactions"))
}

pub fn all(conn: &mut SqliteConnection) -> Vec<User> {
    let nodes: Vec<Node> = nodes_dsl::nodes
        .select(Node::as_select())
        .order(nodes_dsl::created_at_us)
        .load(conn)
        .expect("Error loading nodes");
    
    nodes.into_iter().map(|node| make_user(conn, node)).collect()
}

pub fn ban(conn: &mut SqliteConnection, user: &User) -> QueryResult<User> {
    let account: Account = diesel::update(&user.account)
        .set(accounts_dsl::jackass.eq(true))
        .returning(Account::as_returning())
        .get_result(conn)?;
    Ok(User { account, node: user.node.clone() })
}

pub fn unban(conn: &mut SqliteConnection, user: &User) -> QueryResult<User> {
    let account: Account = diesel::update(&user.account)
        .set(accounts_dsl::jackass.eq(false))
        .returning(Account::as_returning())
        .get_result(conn)?;
    Ok(User { account, node: user.node.clone() })
}

pub fn get(conn: &mut SqliteConnection, node_id: &str) -> QueryResult<User> {
    let node = get_node(conn, node_id)?;
    let account = get_account_by_id(conn, node.account_id)?;
    Ok(User { account, node })
}

/// Get a user by their account id field.
///
/// Note: If the account has multiple nodes, this returns the first one (by id).
/// Use `get_nodes_for_account` if you need all nodes.
pub fn get_by_account_id(conn: &mut SqliteConnection, account_id: i32) -> QueryResult<User> {
    let account = get_account_by_id(conn, account_id)?;
    let node: Node = nodes_dsl::nodes
        .select(Node::as_select())
        .filter(nodes_dsl::account_id.eq(account_id))
        .order(nodes_dsl::id)
        .first(conn)?;
    Ok(User { account, node })
}

/// Get all nodes associated with an account.
pub fn get_nodes_for_account(conn: &mut SqliteConnection, account_id: i32) -> Vec<Node> {
    nodes_dsl::nodes
        .select(Node::as_select())
        .filter(nodes_dsl::account_id.eq(account_id))
        .order(nodes_dsl::id)
        .load(conn)
        .expect("should always be able to load nodes for an account")
}

/// Get just the account by its id (without a node).
pub fn get_account(conn: &mut SqliteConnection, account_id: i32) -> QueryResult<Account> {
    get_account_by_id(conn, account_id)
}

/// Get a user by their short_name field.
///
/// short_names aren't unique. This returns Some(User) if exactly one user has the given
/// short name, or else None.
pub fn get_by_short_name(conn: &mut SqliteConnection, short_name: &str) -> Option<User> {
    let mut nodes: Vec<Node> = nodes_dsl::nodes
        .select(Node::as_select())
        .filter(nodes_dsl::short_name.eq(short_name))
        .load(conn)
        .expect("should always be able to select nodes");
    if nodes.len() == 1 {
        nodes.pop().map(|node| make_user(conn, node))
    } else {
        None
    }
}

pub fn enter_board(conn: &mut SqliteConnection, user: &User, board_id: i32) -> QueryResult<User> {
    let account: Account = diesel::update(&user.account)
        .set(accounts_dsl::in_board.eq(board_id))
        .returning(Account::as_returning())
        .get_result(conn)?;
    Ok(User { account, node: user.node.clone() })
}

pub fn recently_seen(
    conn: &mut SqliteConnection,
    count: i64,
    exclude_node_id: Option<&str>,
) -> Vec<User> {
    // Get nodes ordered by last_seen, then deduplicate by account in code.
    let mut query = nodes_dsl::nodes
        .select(Node::as_select())
        .order(nodes_dsl::last_seen_at_us.desc())
        .into_boxed();

    if let Some(node_id) = exclude_node_id {
        query = query.filter(nodes_dsl::node_id.ne(node_id));
    }

    let nodes: Vec<Node> = query.load(conn).expect("Error loading nodes");

    // Deduplicate by account_id, keeping the first (most recently seen) node per account
    let mut seen_accounts = std::collections::HashSet::new();
    let mut result = Vec::new();
    for node in nodes {
        if seen_accounts.insert(node.account_id) {
            result.push(make_user(conn, node));
            #[allow(clippy::cast_possible_truncation)]
            if result.len() >= count as usize {
                break;
            }
        }
    }
    result
}

pub fn recently_active(
    conn: &mut SqliteConnection,
    count: i64,
    exclude_node_id: Option<&str>,
) -> Vec<User> {
    // Get accounts ordered by last_acted_at_us, then pair each with their most recently seen node.
    let accounts: Vec<Account> = accounts_dsl::accounts
        .select(Account::as_select())
        .filter(accounts_dsl::last_acted_at_us.is_not_null())
        .order(accounts_dsl::last_acted_at_us.desc())
        .load(conn)
        .expect("Error loading accounts");

    // Deduplicate and pair with most recently seen node
    let mut result = Vec::new();
    for account in accounts {
        // Get the most recently seen node for this account, excluding the specified node if any
        let mut node_query = nodes_dsl::nodes
            .select(Node::as_select())
            .filter(nodes_dsl::account_id.eq(account.id))
            .order(nodes_dsl::last_seen_at_us.desc())
            .into_boxed();

        if let Some(node_id) = exclude_node_id {
            node_query = node_query.filter(nodes_dsl::node_id.ne(node_id));
        }

        if let Ok(node) = node_query.first::<Node>(conn) {
            result.push(User { account, node });
            #[allow(clippy::cast_possible_truncation)]
            if result.len() >= count as usize {
                break;
            }
        }
        // If no valid node found (all excluded), skip this account
    }
    result
}

/// Get the number of seen and active users (accounts).
pub fn counts(conn: &mut SqliteConnection) -> (i32, i32) {
    #[allow(clippy::cast_possible_truncation)] // We'll never have more than 4 billion users.
    let seen_users = accounts_dsl::accounts
        .count()
        .get_result::<i64>(conn)
        .expect("Error counting seen users") as i32;
    #[allow(clippy::cast_possible_truncation)] // We'll never have more than 4 billion users.
    let active_users = accounts_dsl::accounts
        .filter(accounts_dsl::last_acted_at_us.is_not_null())
        .count()
        .get_result::<i64>(conn)
        .expect("Error counting active users") as i32;

    (seen_users, active_users)
}

/// Update the user's biography
pub fn update_bio(conn: &mut SqliteConnection, user: &User, bio: &str) -> Result<User> {
    let account_update = AccountUpdate {
        username: None,
        last_acted_at_us: None,
        bio: Some(bio.to_string()),
    };
    account_update.validate()?;

    let account: Account = diesel::update(&user.account)
        .set(accounts_dsl::bio.eq(bio))
        .returning(Account::as_returning())
        .get_result(conn)
        .expect("we must be able to update accounts");
    
    Ok(User { account, node: user.node.clone() })
}

/// Update the user's username
pub fn update_username(conn: &mut SqliteConnection, user: &User, username: Option<&str>) -> Result<User> {
    let account: Account = diesel::update(&user.account)
        .set(accounts_dsl::username.eq(username))
        .returning(Account::as_returning())
        .get_result(conn)
        .expect("we must be able to update accounts");
    
    Ok(User { account, node: user.node.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::thread::sleep;
    use std::time::Duration;

    /// Helper to add a second node to an existing account for testing multi-node scenarios
    fn add_node_to_account(
        conn: &mut SqliteConnection,
        account_id: i32,
        node_id: &str,
    ) -> Node {
        let now = now_as_useconds();
        let new_node = NodeNew {
            account_id,
            node_id,
            short_name: "TST2",
            long_name: "Test Node 2",
            created_at_us: &now,
            last_seen_at_us: &now,
        };
        new_node.validate().expect("new node should be valid");

        diesel::insert_into(nodes::table)
            .values(&new_node)
            .returning(Node::as_returning())
            .get_result(conn)
            .expect("should be able to insert a second node")
    }

    #[test]
    fn record_creates_and_updates_user() {
        let mut conn = db::test_connection();

        let (user, existed) = record(&mut conn, "!00000001").expect("record should succeed");
        assert!(!existed);
        assert_eq!(user.node.short_name, "????");
        let first_seen = user.node.last_seen_at_us;
        let first_acted = user.account.last_acted_at_us.expect("user should have acted");

        let (updated, existed_again) =
            record(&mut conn, "!00000001").expect("record should update user");
        assert!(existed_again);
        assert!(updated.node.last_seen_at_us >= first_seen);
        assert!(
            updated
                .account
                .last_acted_at_us
                .expect("user should still have acted")
                >= first_acted
        );
    }

    #[test]
    fn recently_active_excludes_specified_node() {
        let mut conn = db::test_connection();

        let (_, _) = record(&mut conn, "!00000001").expect("first user");
        sleep(Duration::from_micros(10));
        let (_, _) = record(&mut conn, "!00000002").expect("second user");
        sleep(Duration::from_micros(10));
        let (_, _) = record(&mut conn, "!00000003").expect("third user");

        let active = recently_active(&mut conn, 10, Some("!00000002"));
        let ids: Vec<String> = active.into_iter().map(|u| u.node.node_id).collect();
        assert_eq!(ids, vec!["!00000003".to_string(), "!00000001".to_string()]);
    }

    #[test]
    fn recently_seen_orders_newest_first_and_excludes_requested_node() {
        let mut conn = db::test_connection();

        let (first, _) = record(&mut conn, "!00000011").expect("first user");
        sleep(Duration::from_micros(10));
        let (second, _) = record(&mut conn, "!00000012").expect("second user");
        sleep(Duration::from_micros(10));
        let (third, _) = record(&mut conn, "!00000013").expect("third user");

        let seen = recently_seen(&mut conn, 10, Some(first.node.node_id.as_str()));
        let ids: Vec<String> = seen.into_iter().map(|u| u.node.node_id).collect();
        assert_eq!(ids, vec![third.node.node_id, second.node.node_id]);
    }

    #[test]
    fn recently_seen_deduplicates_multi_node_accounts() {
        let mut conn = db::test_connection();

        // Create first account with one node
        let (user1, _) = record(&mut conn, "!00000021").expect("first user");
        sleep(Duration::from_micros(10));

        // Create second account with two nodes
        let (user2, _) = record(&mut conn, "!00000022").expect("second user");
        sleep(Duration::from_micros(10));
        let node2b = add_node_to_account(&mut conn, user2.account.id, "!00000023");

        // node2b was just created, so it's the most recently seen
        // recently_seen should return only 2 accounts, not 3 nodes
        let seen = recently_seen(&mut conn, 10, None);
        assert_eq!(seen.len(), 2, "should return 2 accounts, not 3 nodes");

        // The most recently seen node for account2 should be node2b
        let account_ids: Vec<i32> = seen.iter().map(|u| u.account.id).collect();
        assert!(account_ids.contains(&user1.account.id));
        assert!(account_ids.contains(&user2.account.id));

        // Account2's entry should show node2b (most recently seen)
        let user2_entry = seen.iter().find(|u| u.account.id == user2.account.id).unwrap();
        assert_eq!(user2_entry.node.node_id, node2b.node_id);
    }

    #[test]
    fn recently_active_deduplicates_multi_node_accounts() {
        let mut conn = db::test_connection();

        // Create first account
        let (_, _) = record(&mut conn, "!00000031").expect("first user");
        sleep(Duration::from_micros(10));

        // Create second account with two nodes
        let (user2, _) = record(&mut conn, "!00000032").expect("second user");
        sleep(Duration::from_micros(10));
        let node2b = add_node_to_account(&mut conn, user2.account.id, "!00000033");

        // recently_active should return only 2 accounts, not 3 nodes
        let active = recently_active(&mut conn, 10, None);
        assert_eq!(active.len(), 2, "should return 2 accounts, not 3 nodes");

        // The most recently seen node for account2 should be node2b
        let user2_entry = active.iter().find(|u| u.account.id == user2.account.id).unwrap();
        assert_eq!(user2_entry.node.node_id, node2b.node_id);
    }

    #[test]
    fn recently_seen_with_excluded_node_falls_back_to_other_node() {
        let mut conn = db::test_connection();

        // Create account with two nodes
        let (user, _) = record(&mut conn, "!00000041").expect("user");
        sleep(Duration::from_micros(10));
        let node_b = add_node_to_account(&mut conn, user.account.id, "!00000042");

        // Exclude the newer node - should fall back to the older one
        let seen = recently_seen(&mut conn, 10, Some(&node_b.node_id));
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].node.node_id, "!00000041");
    }

    #[test]
    fn recently_active_with_excluded_node_falls_back_to_other_node() {
        let mut conn = db::test_connection();

        // Create account with two nodes
        let (user, _) = record(&mut conn, "!00000051").expect("user");
        sleep(Duration::from_micros(10));
        let node_b = add_node_to_account(&mut conn, user.account.id, "!00000052");

        // Exclude the newer node - should fall back to the older one
        let active = recently_active(&mut conn, 10, Some(&node_b.node_id));
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].node.node_id, "!00000051");
    }
}
