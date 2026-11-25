//! File names for captures taken inside the app.

use chrono::NaiveDateTime;

/// `Screenshot YYYY-MM-DD HH.MM.SS.png`, else the `.mmm` form when that is taken.
/// `None` means both are taken: the caller waits 1 ms and retries with a new time.
pub fn capture_file_name(at: NaiveDateTime, taken: impl Fn(&str) -> bool) -> Option<String> {
    let seconds = format!("{}.png", capture_title(at));
    if !taken(&seconds) {
        return Some(seconds);
    }
    let millis = format!(
        "{}.{:03}.png",
        capture_title(at),
        at.and_utc().timestamp_subsec_millis()
    );
    (!taken(&millis)).then_some(millis)
}

/// The editor name of a capture: `Screenshot YYYY-MM-DD HH.MM.SS`.
pub fn capture_title(at: NaiveDateTime) -> String {
    at.format("Screenshot %Y-%m-%d %H.%M.%S").to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Shared test time helper: 2026-11-24 18:42:07 plus `ms` milliseconds.
    pub(crate) fn at(ms: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_milli_opt(18, 42, 7, ms)
            .unwrap()
    }

    #[test]
    fn names_use_seconds_then_milliseconds_and_never_counters() {
        assert_eq!(
            capture_file_name(at(315), |_| false).as_deref(),
            Some("Screenshot 2026-11-24 18.42.07.png")
        );
        let first = |n: &str| n == "Screenshot 2026-11-24 18.42.07.png";
        assert_eq!(
            capture_file_name(at(315), first).as_deref(),
            Some("Screenshot 2026-11-24 18.42.07.315.png")
        );
        assert_eq!(capture_file_name(at(5), |_| true), None);
        assert_eq!(capture_title(at(5)), "Screenshot 2026-11-24 18.42.07");
    }
}
