use super::models::{Invitation, NewInvitation};
use super::schema::invitations::{dsl, table};
use super::{now_as_useconds, Result};
use diesel::prelude::*;
use validator::Validate as _;

/// Invitations expire after 24 hours (in microseconds).
const EXPIRY_US: i64 = 24 * 3600 * 1_000_000;

/// Create a new invitation.
pub fn create(
    conn: &mut SqliteConnection,
    sender_account_id: i32,
    invitee_node_id: i32,
    password: &str,
) -> Result<Invitation> {
    let now = now_as_useconds();
    let new_invitation = NewInvitation {
        sender_account_id,
        invitee_node_id,
        password,
        created_at_us: &now,
    };
    new_invitation.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_invitation)
        .returning(Invitation::as_returning())
        .get_result(conn)
        .expect("should always be able to insert a new invitation"))
}

/// Get an invitation by its ID.
pub fn get_by_id(conn: &mut SqliteConnection, invitation_id: i32) -> QueryResult<Invitation> {
    dsl::invitations
        .select(Invitation::as_select())
        .filter(dsl::id.eq(invitation_id))
        .first(conn)
}

/// Get pending (non-expired, non-accepted, non-denied) invitations sent by an account.
pub fn get_pending_for_sender(
    conn: &mut SqliteConnection,
    sender_account_id: i32,
) -> Vec<Invitation> {
    let cutoff = now_as_useconds() - EXPIRY_US;
    dsl::invitations
        .select(Invitation::as_select())
        .filter(dsl::sender_account_id.eq(sender_account_id))
        .filter(dsl::accepted_at_us.is_null())
        .filter(dsl::denied_at_us.is_null())
        .filter(dsl::created_at_us.gt(cutoff))
        .order(dsl::created_at_us.desc())
        .load(conn)
        .expect("should always be able to query invitations")
}

/// Get pending (non-expired, non-accepted, non-denied) invitations targeting a specific node.
pub fn get_pending_for_invitee(
    conn: &mut SqliteConnection,
    invitee_node_id: i32,
) -> Vec<Invitation> {
    let cutoff = now_as_useconds() - EXPIRY_US;
    dsl::invitations
        .select(Invitation::as_select())
        .filter(dsl::invitee_node_id.eq(invitee_node_id))
        .filter(dsl::accepted_at_us.is_null())
        .filter(dsl::denied_at_us.is_null())
        .filter(dsl::created_at_us.gt(cutoff))
        .order(dsl::created_at_us.desc())
        .load(conn)
        .expect("should always be able to query invitations")
}

/// Check if any pending (non-accepted, non-denied) invitation exists for an invitee,
/// regardless of expiry. Used to distinguish expired invitations from non-existent ones.
pub fn get_any_pending_for_invitee(
    conn: &mut SqliteConnection,
    invitee_node_id: i32,
) -> Vec<Invitation> {
    dsl::invitations
        .select(Invitation::as_select())
        .filter(dsl::invitee_node_id.eq(invitee_node_id))
        .filter(dsl::accepted_at_us.is_null())
        .filter(dsl::denied_at_us.is_null())
        .order(dsl::created_at_us.desc())
        .load(conn)
        .expect("should always be able to query invitations")
}

/// Mark an invitation as accepted.
pub fn accept(conn: &mut SqliteConnection, invitation: &Invitation) -> QueryResult<Invitation> {
    let now = now_as_useconds();
    diesel::update(invitation)
        .set(dsl::accepted_at_us.eq(now))
        .returning(Invitation::as_returning())
        .get_result(conn)
}

/// Get the most recent invitation sent by an account (any status, including expired).
/// Used for rate limiting.
pub fn get_most_recent_for_sender(
    conn: &mut SqliteConnection,
    sender_account_id: i32,
) -> Option<Invitation> {
    dsl::invitations
        .select(Invitation::as_select())
        .filter(dsl::sender_account_id.eq(sender_account_id))
        .order(dsl::created_at_us.desc())
        .first(conn)
        .ok()
}

/// Create a new invitation with a specific timestamp (for testing rate limits and expiry).
pub fn create_with_timestamp(
    conn: &mut SqliteConnection,
    sender_account_id: i32,
    invitee_node_id: i32,
    password: &str,
    created_at_us: i64,
) -> Result<Invitation> {
    let new_invitation = NewInvitation {
        sender_account_id,
        invitee_node_id,
        password,
        created_at_us: &created_at_us,
    };
    new_invitation.validate()?;

    Ok(diesel::insert_into(table)
        .values(&new_invitation)
        .returning(Invitation::as_returning())
        .get_result(conn)
        .expect("should always be able to insert a new invitation"))
}

/// Delete all pending (non-expired, non-accepted, non-denied) outbound invitations for an account.
/// Used when an invitee accepts, to clean up any outbound invitation they may have sent.
pub fn delete_pending_for_sender(conn: &mut SqliteConnection, sender_account_id: i32) -> usize {
    let cutoff = now_as_useconds() - EXPIRY_US;
    diesel::delete(
        dsl::invitations
            .filter(dsl::sender_account_id.eq(sender_account_id))
            .filter(dsl::accepted_at_us.is_null())
            .filter(dsl::denied_at_us.is_null())
            .filter(dsl::created_at_us.gt(cutoff)),
    )
    .execute(conn)
    .expect("should always be able to delete invitations")
}

/// Mark an invitation as denied.
pub fn deny(conn: &mut SqliteConnection, invitation: &Invitation) -> QueryResult<Invitation> {
    let now = now_as_useconds();
    diesel::update(invitation)
        .set(dsl::denied_at_us.eq(now))
        .returning(Invitation::as_returning())
        .get_result(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::models::{AccountNew, NodeNew};
    use crate::db::now_as_useconds;
    use crate::db::schema::{accounts, nodes};

    /// Helper: create an account and return its id.
    fn create_account(conn: &mut SqliteConnection) -> i32 {
        let now = now_as_useconds();
        let new_account = AccountNew {
            username: None,
            created_at_us: &now,
            last_acted_at_us: None,
            invite_allowed: false,
        };
        new_account.validate().expect("valid account");
        diesel::insert_into(accounts::table)
            .values(&new_account)
            .returning(accounts::dsl::id)
            .get_result(conn)
            .expect("should insert account")
    }

    /// Helper: create a node for an account and return its id.
    fn create_node(conn: &mut SqliteConnection, account_id: i32, node_id: &str) -> i32 {
        let now = now_as_useconds();
        let new_node = NodeNew {
            account_id,
            node_id,
            short_name: "TEST",
            long_name: "Test Node",
            created_at_us: &now,
            last_seen_at_us: &now,
        };
        new_node.validate().expect("valid node");
        diesel::insert_into(nodes::table)
            .values(&new_node)
            .returning(nodes::dsl::id)
            .get_result(conn)
            .expect("should insert node")
    }

    #[test]
    fn create_and_get_by_id() {
        let mut conn = db::test_connection();
        let account_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0001");

        let invitation = create(&mut conn, account_id, node_id, "testpass123")
            .expect("should create invitation");
        assert_eq!(invitation.sender_account_id, account_id);
        assert_eq!(invitation.invitee_node_id, node_id);
        assert_eq!(invitation.password, "testpass123");
        assert!(invitation.accepted_at_us.is_none());
        assert!(invitation.denied_at_us.is_none());

        let fetched = get_by_id(&mut conn, invitation.id).expect("should find invitation");
        assert_eq!(fetched.id, invitation.id);
        assert_eq!(fetched.password, "testpass123");
    }

    #[test]
    fn get_pending_for_sender_returns_active_invitations() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0002");

        let _inv = create(&mut conn, sender_id, node_id, "pass1").expect("should create");

        let pending = get_pending_for_sender(&mut conn, sender_id);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].password, "pass1");
    }

    #[test]
    fn get_pending_for_sender_excludes_accepted() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0003");

        let inv = create(&mut conn, sender_id, node_id, "pass2").expect("should create");
        accept(&mut conn, &inv).expect("should accept");

        let pending = get_pending_for_sender(&mut conn, sender_id);
        assert!(pending.is_empty());
    }

    #[test]
    fn get_pending_for_sender_excludes_denied() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0004");

        let inv = create(&mut conn, sender_id, node_id, "pass3").expect("should create");
        deny(&mut conn, &inv).expect("should deny");

        let pending = get_pending_for_sender(&mut conn, sender_id);
        assert!(pending.is_empty());
    }

    #[test]
    fn get_pending_for_sender_excludes_expired() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0005");

        // Insert invitation with an old timestamp (>24 hours ago)
        let old_time = now_as_useconds() - EXPIRY_US - 1_000_000;
        let new_inv = NewInvitation {
            sender_account_id: sender_id,
            invitee_node_id: node_id,
            password: "pass4",
            created_at_us: &old_time,
        };
        new_inv.validate().expect("valid invitation");
        diesel::insert_into(table)
            .values(&new_inv)
            .execute(&mut conn)
            .expect("should insert old invitation");

        let pending = get_pending_for_sender(&mut conn, sender_id);
        assert!(pending.is_empty());
    }

    #[test]
    fn get_pending_for_invitee_returns_active_invitations() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0006");

        let _inv = create(&mut conn, sender_id, node_id, "pass5").expect("should create");

        let pending = get_pending_for_invitee(&mut conn, node_id);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].password, "pass5");
    }

    #[test]
    fn get_pending_for_invitee_excludes_accepted() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0007");

        let inv = create(&mut conn, sender_id, node_id, "pass6").expect("should create");
        accept(&mut conn, &inv).expect("should accept");

        let pending = get_pending_for_invitee(&mut conn, node_id);
        assert!(pending.is_empty());
    }

    #[test]
    fn get_pending_for_invitee_excludes_denied() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0008");

        let inv = create(&mut conn, sender_id, node_id, "pass7").expect("should create");
        deny(&mut conn, &inv).expect("should deny");

        let pending = get_pending_for_invitee(&mut conn, node_id);
        assert!(pending.is_empty());
    }

    #[test]
    fn get_pending_for_invitee_excludes_expired() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0009");

        let old_time = now_as_useconds() - EXPIRY_US - 1_000_000;
        let new_inv = NewInvitation {
            sender_account_id: sender_id,
            invitee_node_id: node_id,
            password: "pass8",
            created_at_us: &old_time,
        };
        new_inv.validate().expect("valid invitation");
        diesel::insert_into(table)
            .values(&new_inv)
            .execute(&mut conn)
            .expect("should insert old invitation");

        let pending = get_pending_for_invitee(&mut conn, node_id);
        assert!(pending.is_empty());
    }

    #[test]
    fn accept_sets_accepted_at_us() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0010");

        let inv = create(&mut conn, sender_id, node_id, "pass9").expect("should create");
        assert!(inv.accepted_at_us.is_none());

        let accepted = accept(&mut conn, &inv).expect("should accept");
        assert!(accepted.accepted_at_us.is_some());
        assert!(accepted.denied_at_us.is_none());
    }

    #[test]
    fn deny_sets_denied_at_us() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0011");

        let inv = create(&mut conn, sender_id, node_id, "pass10").expect("should create");
        assert!(inv.denied_at_us.is_none());

        let denied = deny(&mut conn, &inv).expect("should deny");
        assert!(denied.denied_at_us.is_some());
        assert!(denied.accepted_at_us.is_none());
    }

    #[test]
    fn multiple_invitations_for_different_invitees() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_1 = create_account(&mut conn);
        let invitee_account_2 = create_account(&mut conn);
        let node_id_1 = create_node(&mut conn, invitee_account_1, "!aabb0012");
        let node_id_2 = create_node(&mut conn, invitee_account_2, "!aabb0013");

        let _inv1 = create(&mut conn, sender_id, node_id_1, "pass_a").expect("should create");
        let _inv2 = create(&mut conn, sender_id, node_id_2, "pass_b").expect("should create");

        let pending = get_pending_for_sender(&mut conn, sender_id);
        assert_eq!(pending.len(), 2);

        let pending_1 = get_pending_for_invitee(&mut conn, node_id_1);
        assert_eq!(pending_1.len(), 1);
        assert_eq!(pending_1[0].password, "pass_a");

        let pending_2 = get_pending_for_invitee(&mut conn, node_id_2);
        assert_eq!(pending_2.len(), 1);
        assert_eq!(pending_2[0].password, "pass_b");
    }

    #[test]
    fn invitation_created_at_formats_timestamp() {
        let mut conn = db::test_connection();
        let sender_id = create_account(&mut conn);
        let invitee_account_id = create_account(&mut conn);
        let node_id = create_node(&mut conn, invitee_account_id, "!aabb0014");

        let inv = create(&mut conn, sender_id, node_id, "pass11").expect("should create");
        let formatted = inv.created_at();
        assert!(formatted.starts_with("20"));
        assert!(formatted.contains('T'));
    }

    #[test]
    fn new_invitation_validates_fields() {
        let zero_time: i64 = 0;
        let inv = NewInvitation {
            sender_account_id: 0,
            invitee_node_id: 0,
            password: "",
            created_at_us: &zero_time,
        };
        assert!(
            inv.validate().is_err(),
            "invalid fields should fail validation"
        );
    }
}
