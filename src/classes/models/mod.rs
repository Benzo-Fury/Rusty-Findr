pub mod job;
pub mod index;
pub mod torrent;

/// SQL fragment to cast a TIMESTAMPTZ column to an ISO 8601 string.
macro_rules! ts {
    ($col:expr) => {
        concat!("to_char(", $col, " AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')")
    };
}
pub(crate) use ts;