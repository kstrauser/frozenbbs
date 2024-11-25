pub mod admin;
pub mod db;

use time::{format_description::BorrowedFormatItem, macros::format_description};

const TSTAMP_FORMAT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]@[hour]:[minute]:[second]");

pub fn formatted_tstamp(tstamp: time::PrimitiveDateTime) -> String {
    tstamp.format(&TSTAMP_FORMAT).unwrap()
}
