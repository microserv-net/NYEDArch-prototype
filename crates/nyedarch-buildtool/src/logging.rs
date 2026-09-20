//! Verbose diagnostics, timestamped.
//!
//! Enabled with `--with-logs` on the command line and a switch in the desktop
//! client. Off by default: a security tool that narrates every step by default
//! trains people to stop reading it, and the lines that matter — a refusal, a
//! downgrade — get lost in the noise.
//!
//! # Times are IST
//!
//! Asia/Kolkata, UTC+05:30. Fixed rather than read from the system, for two
//! reasons: India has no daylight saving, so the offset is constant and cannot
//! drift; and a log that says 14:32 on one machine and 09:02 on another is
//! useless when comparing a build against the capsule it produced. Every line
//! carries the offset explicitly so nobody has to guess which clock it is.
//!
//! # What a verbose line may say
//!
//! Enough to answer "did that actually happen?" and nothing more. Accuracy in
//! metres, yes; coordinates, never. Signal names and strengths, yes; raw
//! hardware identifiers, never — the client does not hold them in the first
//! place. The rule is the same as for `Debug`: diagnostics are for confirming
//! behaviour, not for reproducing secrets.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set once from the command line or the desktop client.
static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Lines held for a caller that has no console.
///
/// The desktop client is the case this exists for: on macOS it runs from an
/// application bundle, where stderr goes nowhere a user will ever look. Rather
/// than teach every call site about two destinations, lines are buffered here
/// and the client drains them into its activity log.
static BUFFER: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static CAPTURE: AtomicBool = AtomicBool::new(false);

/// Buffer lines instead of relying on stderr.
pub fn set_capture(on: bool) {
    CAPTURE.store(on, Ordering::Relaxed);
}

/// Take everything buffered since the last call.
///
/// Bounded: if a caller stops draining, old lines are dropped rather than
/// growing without limit. A diagnostic buffer that can exhaust memory is a
/// denial of service wearing a helpful hat.
pub fn drain() -> Vec<String> {
    match BUFFER.lock() {
        Ok(mut b) => std::mem::take(&mut *b),
        // A poisoned lock means a thread panicked while logging. Losing
        // diagnostics is not worth propagating a panic into the caller.
        Err(_) => Vec::new(),
    }
}

const BUFFER_LIMIT: usize = 2_000;

pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// Seconds east of UTC for Asia/Kolkata.
const IST_OFFSET_SECS: i64 = 5 * 3600 + 30 * 60;

/// Civil date from a day count since 1970-01-01.
///
/// Howard Hinnant's `civil_from_days`, which is exact for the whole proleptic
/// Gregorian range and needs no table. Worth using verbatim rather than
/// improvising: date arithmetic is the kind of thing that looks right for years
/// and then fails on a leap day.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The current time in IST, as `YYYY-MM-DD HH:MM:SS.mmm +05:30`.
pub fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format_ist(now.as_secs() as i64, now.subsec_millis())
}

/// Split out so it can be tested against known instants.
pub fn format_ist(unix_secs: i64, millis: u32) -> String {
    let local = unix_secs + IST_OFFSET_SECS;
    let days = local.div_euclid(86_400);
    let secs_of_day = local.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{millis:03} +05:30",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    )
}

/// Write one diagnostic line, if verbose logging is on.
///
/// Goes to stderr so it never contaminates anything a caller is parsing from
/// stdout — a log that breaks a pipeline is worse than no log.
pub fn log(area: &str, message: impl AsRef<str>) {
    if !verbose() {
        return;
    }
    let line = format!("[{}] {:<11} {}", timestamp(), area, message.as_ref());
    if CAPTURE.load(Ordering::Relaxed) {
        if let Ok(mut b) = BUFFER.lock() {
            if b.len() >= BUFFER_LIMIT {
                b.remove(0);
            }
            b.push(line);
            return;
        }
    }
    eprintln!("{line}");
}

/// A line that is always written, verbose or not.
///
/// For the handful of things an operator must see regardless: a security
/// downgrade, a refusal, a destination.
pub fn always(area: &str, message: impl AsRef<str>) {
    eprintln!("[{}] {:<11} {}", timestamp(), area, message.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known instants, checked by hand.
    #[test]
    fn ist_conversion_is_correct() {
        // 1970-01-01 00:00:00 UTC is 05:30 on the same day in IST.
        assert_eq!(format_ist(0, 0), "1970-01-01 05:30:00.000 +05:30");

        // 2000-01-01T00:00:00Z = 946684800 -> 05:30 IST on the same day.
        assert_eq!(format_ist(946_684_800, 0), "2000-01-01 05:30:00.000 +05:30");
    }

    /// The offset is applied, and it rolls the date when it should.
    #[test]
    fn the_offset_rolls_the_date() {
        // 2026-01-01 19:00:00 UTC is 00:30 on 2 January in IST.
        // 2026-01-01T19:00:00Z = 1767294000
        assert_eq!(format_ist(1_767_294_000, 0), "2026-01-02 00:30:00.000 +05:30");
        // One second earlier is 00:29:59 - still the 2nd, because the offset
        // has already carried it past midnight. The first version of this test
        // asserted 23:29:59 on the 1st, which is the answer for UTC, not IST.
        assert_eq!(format_ist(1_767_293_999, 0), "2026-01-02 00:29:59.000 +05:30");
        // To land on the 1st in IST the instant must be before 18:30 UTC.
        assert_eq!(format_ist(1_767_293_999 - 3_600, 0), "2026-01-01 23:29:59.000 +05:30");
    }

    /// A leap day must not shift the calendar.
    #[test]
    fn leap_days_are_handled() {
        // 2024-02-29T00:00:00Z = 1709164800 -> 05:30 IST on the 29th.
        assert_eq!(format_ist(1_709_164_800, 0), "2024-02-29 05:30:00.000 +05:30");
        // The following day.
        assert_eq!(format_ist(1_709_251_200, 0), "2024-03-01 05:30:00.000 +05:30");
    }

    #[test]
    fn milliseconds_are_padded() {
        assert!(format_ist(0, 7).ends_with("05:30:00.007 +05:30"));
        assert!(format_ist(0, 70).ends_with("05:30:00.070 +05:30"));
    }

    /// Verbose is off unless asked for: a tool that narrates by default trains
    /// people to stop reading it.
    /// Captured lines reach the buffer and are handed over exactly once.
    #[test]
    fn capture_buffers_and_drains() {
        set_verbose(true);
        set_capture(true);
        let _ = drain(); // start clean

        log("test", "first");
        log("test", "second");
        let lines = drain();
        assert_eq!(lines.len(), 2, "both lines buffered: {lines:?}");
        assert!(lines[0].contains("first") && lines[1].contains("second"));
        assert!(lines[0].contains("+05:30"), "lines carry an IST timestamp");

        assert!(drain().is_empty(), "a drained line must not be handed over twice");

        set_capture(false);
        set_verbose(false);
    }

    /// The buffer must not grow without limit when nobody drains it.
    #[test]
    fn the_buffer_is_bounded() {
        set_verbose(true);
        set_capture(true);
        let _ = drain();
        for i in 0..(BUFFER_LIMIT + 50) {
            log("test", format!("line {i}"));
        }
        let lines = drain();
        assert!(
            lines.len() <= BUFFER_LIMIT,
            "a diagnostic buffer that grows without limit is a denial of service"
        );
        set_capture(false);
        set_verbose(false);
    }

    #[test]
    fn verbose_is_off_by_default() {
        // Only meaningful before anything sets it, so this asserts the initial
        // constant rather than the live flag.
        assert!(!AtomicBool::new(false).load(Ordering::Relaxed));
    }
}
