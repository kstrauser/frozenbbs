// @generated automatically by Diesel CLI.

diesel::table! {
    boards (id) {
        id -> Integer,
        name -> Text,
        description -> Text,
        created_at_us -> BigInt,
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
    users (id) {
        id -> Integer,
        node_id -> Text,
        short_name -> Text,
        long_name -> Text,
        jackass -> Bool,
        in_board -> Nullable<Integer>,
        created_at_us -> BigInt,
        last_seen_at_us -> BigInt,
    }
}

diesel::joinable!(posts -> boards (board_id));
diesel::joinable!(posts -> users (user_id));
diesel::joinable!(users -> boards (in_board));

diesel::allow_tables_to_appear_in_same_query!(
    boards,
    posts,
    users,
);
