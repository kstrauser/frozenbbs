use super::{Replies, ERROR_POSTING};
use crate::db::{board_states, boards, posts, users, Post, User};
use crate::{linefeed, BBSConfig};
use diesel::SqliteConnection;

const NOT_IN_BOARD: &str = "You are not in a board.";
const NOT_VALID: &str = "That's a valid number.";
const NO_BOARDS: &str = "There are no boards.";
const NO_MORE_POSTS: &str = "There are no more posts in this board.";
const NO_MORE_UNREAD: &str = "There are no more unread posts in any board.";
const NO_SUCH_POST: &str = "There is no post here.";

/// Does this board have any unread posts for this user?
fn has_unread(conn: &mut SqliteConnection, user: &User, board_id: i32) -> bool {
    let last_seen = board_states::get(conn, user.id, board_id);
    posts::after(conn, board_id, last_seen).is_ok()
}

/// Print a post and information about its author.
fn post_print(post: &Post, user: &User) -> Vec<String> {
    let mut out = vec![
        format!("From: {}", user),
        format!("At: {}", post.created_at()),
    ];
    // Split individual lines into separate strings to help the paginator deal with longer chunks.
    for line in post.body.split("\n") {
        linefeed!(out);
        out.push(line.to_string());
    }
    out
}

/// List all the boards.
pub fn lister(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let all_boards = boards::all(conn);
    if all_boards.is_empty() {
        return NO_BOARDS.into();
    }
    let mut out = Vec::new();
    out.push("Boards:".to_string());
    linefeed!(out);
    let mut any_unread = false;
    for board in &all_boards {
        let unread_here = has_unread(conn, user, board.id);
        if unread_here {
            any_unread = true;
        }
        let mut prefix = String::new();
        if unread_here {
            prefix += "!";
        }
        if user.in_board.is_some() && user.in_board.unwrap() == board.id {
            prefix += "*";
        }
        if !prefix.is_empty() {
            prefix += " ";
        }
        out.push(format!("{prefix}{board}"));
    }
    if user.in_board.is_some() {
        linefeed!(out);
        out.push("* You are here.".to_string());
        if any_unread {
            out.push("! Board has unread messages.".to_string());
        }
    }
    out.into()
}

/// Enter a board.
#[allow(clippy::needless_pass_by_value)]
pub fn enter(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(num) = args.get(1) else {
        return NOT_VALID.into();
    };
    let Ok(num) = num.parse::<i32>() else {
        return NOT_VALID.into();
    };
    let count = boards::count(conn);
    if count == 0 {
        return NO_BOARDS.into();
    }
    if num < 1 || num > count {
        return format!("Board number must be between 1 and {count}").into();
    }
    let _ = users::enter_board(conn, user, num);
    let board = boards::get(conn, num).expect("we should find a board that we already know exists");
    format!("Entering board {num}, {}.", board.name).into()
}

/// Get the current message in the board.
pub fn current(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let Some(in_board) = user.in_board else {
        return NOT_IN_BOARD.into();
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::current(conn, in_board, last_seen) {
        post_print(&post, &post_user).into()
    } else {
        NO_SUCH_POST.into()
    }
}

/// Get the previous message in the board.
pub fn previous(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let Some(in_board) = user.in_board else {
        return NOT_IN_BOARD.into();
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::before(conn, in_board, last_seen) {
        board_states::update(conn, user.id, in_board, post.created_at_us);
        post_print(&post, &post_user).into()
    } else {
        NO_MORE_POSTS.into()
    }
}

/// Get the next message in the board.
pub fn next(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let Some(in_board) = user.in_board else {
        return NOT_IN_BOARD.into();
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((post, post_user)) = posts::after(conn, in_board, last_seen) {
        board_states::update(conn, user.id, in_board, post.created_at_us);
        post_print(&post, &post_user).into()
    } else {
        NO_MORE_POSTS.into()
    }
}

///Get the next unread message in any board.
pub fn quick(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    // General note about this method: it's not terribly efficient and makes repeated calls to the
    // database to get information it could fetch in some more complex joins. I highly, *highly*
    // doubt this will ever be a performance issue, given how inherently small the related data is
    // (it scales with the total number of boards which probably isn't going to be in the
    // millions). The naive approach means the code is a lot simpler and easier to reason about,
    // and avoids the common case where we'd be fetching *all* the data and then ignoring most
    // of it.

    let in_board = user.in_board.unwrap_or(1);
    // Make a series of board numbers, starting where the user currently is and going to the last,
    // then starting at the beginning and back to just before where the user started.
    //
    // That way they'll see everything in this board, then everything in the next, then the next,
    // and wrap around at the first board and keep going.
    let mut board_nums: Vec<i32> = (1..=boards::count(conn)).collect();
    board_nums.rotate_left(in_board as usize - 1);

    let mut out = vec![];
    for board_num in board_nums {
        let last_seen = board_states::get(conn, user.id, board_num);
        if let Ok((post, post_user)) = posts::after(conn, board_num, last_seen) {
            if user.in_board.is_none() || board_num != in_board {
                let _ = users::enter_board(conn, user, board_num);
                // Let the user know they're moving to a different board to read the new post.
                let board = boards::get(conn, board_num).expect("this board should exist");
                out.push(format!("In {}:", board.name));
                linefeed!(out);
            }
            board_states::update(conn, user.id, board_num, post.created_at_us);
            out.extend(post_print(&post, &post_user));
            return out.into();
        }
    }

    NO_MORE_UNREAD.into()
}

/// Add a new post to the board.
#[allow(clippy::needless_pass_by_value)]
pub fn write(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    args: Vec<&str>,
) -> Replies {
    let Some(in_board) = user.in_board else {
        return NOT_IN_BOARD.into();
    };
    let Some(body) = args.get(1) else {
        return ERROR_POSTING.into();
    };
    let Ok(post) = posts::add(conn, user.id, in_board, body) else {
        log::error!("User {user} was unable to post {args:?} to {in_board}.");
        return ERROR_POSTING.into();
    };
    format!("Published at {}", post.created_at()).into()
}

/// Show information about the current post's author
#[allow(clippy::needless_pass_by_value)]
pub fn author(
    conn: &mut SqliteConnection,
    _cfg: &BBSConfig,
    user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let Some(in_board) = user.in_board else {
        return NOT_IN_BOARD.into();
    };
    let last_seen = board_states::get(conn, user.id, in_board);
    if let Ok((_, post_user)) = posts::current(conn, in_board, last_seen) {
        let mut out = vec![
            format!("This post was written by {post_user}."),
            format!("Last seen: {}", user.last_seen_at()),
            format!("Last active: {}", user.last_acted_at()),
        ];
        if let Some(bio) = &post_user.bio {
            if !bio.is_empty() {
                linefeed!(out);
                out.push("Bio:".to_string());
                out.push(bio.to_string());
            }
        }
        out.into()
    } else {
        NO_SUCH_POST.into()
    }
}
