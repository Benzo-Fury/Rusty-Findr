use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC time formatted as "YYYY-MM-DD HH:MM:SS" for log files.
pub fn now_log_timestamp() -> String {
    let secs = unix_now();
    let (y, mo, d, h, mi, s) = unix_to_parts(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// Current UTC date formatted as "YYYY-MM-DD" for API parameters.
pub fn today_ymd() -> String {
    let secs = unix_now();
    let (y, mo, d, _, _, _) = unix_to_parts(secs);
    format!("{y:04}-{mo:02}-{d:02}")
}

/// UTC date N days in the past, formatted as "YYYY-MM-DD".
pub fn days_ago_ymd(days: u64) -> String {
    let secs = unix_now().saturating_sub(days * 86_400);
    let (y, mo, d, _, _, _) = unix_to_parts(secs);
    format!("{y:04}-{mo:02}-{d:02}")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock before Unix epoch")
        .as_secs()
}

/// Convert Unix timestamp to (year, month, day, hour, minute, second).
/// Uses Howard Hinnant's civil_from_days algorithm.
fn unix_to_parts(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;

    // civil_from_days: days since 1970-01-01 -> (y, m, d)
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let hour = (time_of_day / 3600) as u32;
    let minute = ((time_of_day % 3600) / 60) as u32;
    let second = (time_of_day % 60) as u32;

    (y as i32, m, d, hour, minute, second)
}
