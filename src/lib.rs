pub mod admin;
pub mod client;
pub mod db;
use chrono::{Local, TimeZone, Utc};

/// Get the number of microseconds since the Unix epoch.
pub fn now_as_useconds() -> i64 {
    Utc::now().timestamp_micros()
}

/// Format the number of microseconds since the Unix epoch as a local timestamp.
pub fn formatted_useconds(dstamp: i64) -> String {
    format!(
        "{}",
        Local
            .timestamp_micros(dstamp)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S")
    )
}
