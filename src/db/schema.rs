// @generated automatically by Diesel CLI.

diesel::table! {
    accounts (id) {
        id -> Integer,
        username -> Nullable<Text>,
        jackass -> Bool,
        bio -> Nullable<Text>,
        created_at_us -> BigInt,
        last_acted_at_us -> Nullable<BigInt>,
    }
}

diesel::table! {
    board_states (id) {
        id -> Integer,
        user_id -> Integer,
        board_id -> Integer,
        last_post_us -> BigInt,
    }
}

diesel::table! {
    boards (id) {
        id -> Integer,
        name -> Text,
        description -> Text,
        created_at_us -> BigInt,
    }
}

diesel::table! {
    nodes (id) {
        id -> Integer,
        account_id -> Integer,
        node_id -> Text,
        short_name -> Text,
        long_name -> Text,
        in_board -> Nullable<Integer>,
        created_at_us -> BigInt,
        last_seen_at_us -> BigInt,
    }
}

diesel::table! {
    posts (id) {
        id -> Integer,
        board_id -> Integer,
        user_id -> Integer,
        body -> Text,
        created_at_us -> BigInt,
    }
}

diesel::table! {
    queued_messages (id) {
        id -> Integer,
        sender_id -> Integer,
        recipient_id -> Integer,
        body -> Text,
        created_at_us -> BigInt,
        sent_at_us -> Nullable<BigInt>,
    }
}

diesel::joinable!(board_states -> accounts (user_id));
diesel::joinable!(board_states -> boards (board_id));
diesel::joinable!(nodes -> accounts (account_id));
diesel::joinable!(nodes -> boards (in_board));
diesel::joinable!(posts -> accounts (user_id));
diesel::joinable!(posts -> boards (board_id));

diesel::allow_tables_to_appear_in_same_query!(
    accounts,
    board_states,
    boards,
    nodes,
    posts,
    queued_messages,
);
