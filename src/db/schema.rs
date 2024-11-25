// @generated automatically by Diesel CLI.

diesel::table! {
    boards (id) {
        id -> Integer,
        name -> Text,
        description -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    posts (id) {
        id -> Integer,
        board_id -> Integer,
        user_id -> Integer,
        body -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        node_id -> Text,
        short_name -> Text,
        long_name -> Text,
        created_at -> Timestamp,
        last_seen_at -> Timestamp,
    }
}

diesel::joinable!(posts -> boards (board_id));
diesel::joinable!(posts -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    boards,
    posts,
    users,
);
