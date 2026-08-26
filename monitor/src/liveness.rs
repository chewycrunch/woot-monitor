//! Liveness signal: the timestamp of the last successful catalogue read, and
//! the staleness margin the health check judges it against.

use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Outside /app, which is root-owned and holds the read-only config mount.
pub const SIGNAL_PATH: &str = "/run/woot-monitor/alive";

/// Missed polls tolerated before the signal reads stale.
const MARGIN_INTERVALS: u32 = 3;

/// Keeps a short interval from turning one slow read into an alert.
const MARGIN_FLOOR: Duration = Duration::from_secs(60);

// @spec DETECTION-065
/// How long a signal stays fresh, given the configured poll interval.
pub fn margin(delay: Duration) -> Duration {
    delay
        .saturating_mul(MARGIN_INTERVALS)
        .saturating_add(MARGIN_FLOOR)
}

// @spec DETECTION-060, DETECTION-061, DETECTION-065
/// Writes the signal, replacing any previous one.
pub fn record(path: &Path, now: SystemTime, delay: Duration) -> io::Result<()> {
    let written = now
        .duration_since(UNIX_EPOCH)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let line = format!("{} {}", written.as_secs(), margin(delay).as_secs());

    // Renamed into place so a check never reads a half-written line.
    let staging = path.with_extension("tmp");
    fs::write(&staging, line)?;
    fs::rename(&staging, path)
}

// @spec DETECTION-063, DETECTION-064
/// Whether the signal at `path` is recent enough to call the monitor healthy.
/// An absent, unreadable, or malformed signal is not.
pub fn check(path: &Path, now: SystemTime) -> bool {
    let (Ok(contents), Ok(now)) = (fs::read_to_string(path), now.duration_since(UNIX_EPOCH)) else {
        return false;
    };

    is_fresh(&contents, now.as_secs())
}

// @spec DETECTION-063
fn is_fresh(contents: &str, now_secs: u64) -> bool {
    let mut fields = contents.split_whitespace().map(str::parse::<u64>);
    let (Some(Ok(written)), Some(Ok(margin))) = (fields.next(), fields.next()) else {
        return false;
    };

    // A clock that stepped backwards saturates to zero rather than to stale.
    now_secs.saturating_sub(written) <= margin
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Unique per test and per process, so a parallel run cannot collide.
    fn scratch_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("woot-liveness-{}-{name}", std::process::id()))
    }

    // @spec DETECTION-065
    #[test]
    fn the_margin_scales_with_the_poll_interval() {
        assert_eq!(margin(Duration::from_secs(5)), Duration::from_secs(75));
        assert_eq!(margin(Duration::from_secs(60)), Duration::from_secs(240));
    }

    // @spec DETECTION-065
    /// Three times a sub-second interval is shorter than a single slow read.
    #[test]
    fn the_margin_floor_covers_short_intervals() {
        assert!(margin(Duration::from_millis(500)) >= MARGIN_FLOOR);
    }

    // @spec DETECTION-063
    #[test]
    fn a_recent_signal_is_fresh() {
        assert!(is_fresh("1000 75", 1030));
    }

    // @spec DETECTION-063
    #[test]
    fn a_signal_older_than_its_margin_is_stale() {
        assert!(!is_fresh("1000 75", 1076));
    }

    // @spec DETECTION-063
    /// "Older than the margin" leaves a signal aged exactly the margin fresh.
    #[test]
    fn a_signal_aged_exactly_the_margin_is_fresh() {
        assert!(is_fresh("1000 75", 1075));
    }

    // @spec DETECTION-063
    /// A clock that has stepped backwards is not evidence of a dead monitor.
    #[test]
    fn a_signal_from_the_future_is_fresh() {
        assert!(is_fresh("2000 75", 1000));
    }

    // @spec DETECTION-063
    #[test]
    fn a_malformed_signal_is_not_fresh() {
        for contents in ["", "1000", "1000 ", "1000 abc", "abc 75", "not a signal"] {
            assert!(
                !is_fresh(contents, 1000),
                "{contents:?} should not read as fresh"
            );
        }
    }

    // @spec DETECTION-064
    /// Nothing writes the signal until the first read succeeds.
    #[test]
    fn an_absent_signal_is_never_healthy() {
        let path = scratch_path("absent");
        fs::remove_file(&path).ok();
        assert!(!check(&path, SystemTime::now()));
    }

    // @spec DETECTION-060, DETECTION-065
    #[test]
    fn a_recorded_signal_reads_back_fresh() {
        let path = scratch_path("roundtrip");
        record(&path, SystemTime::now(), Duration::from_secs(5)).expect("record");
        assert!(check(&path, SystemTime::now()));
        fs::remove_file(&path).ok();
    }

    // @spec DETECTION-063
    #[test]
    fn a_signal_left_behind_by_a_stalled_monitor_reads_unhealthy() {
        let path = scratch_path("stale");
        let long_ago = SystemTime::now() - Duration::from_secs(3600);
        record(&path, long_ago, Duration::from_secs(5)).expect("record");
        assert!(!check(&path, SystemTime::now()));
        fs::remove_file(&path).ok();
    }

    // @spec DETECTION-060
    /// Each read replaces the signal, so the file never accumulates lines.
    #[test]
    fn recording_replaces_the_previous_signal() {
        let path = scratch_path("replace");
        record(
            &path,
            SystemTime::now() - Duration::from_secs(3600),
            Duration::from_secs(5),
        )
        .expect("record");
        record(&path, SystemTime::now(), Duration::from_secs(5)).expect("record");

        assert!(check(&path, SystemTime::now()));
        assert_eq!(fs::read_to_string(&path).expect("read").lines().count(), 1);
        fs::remove_file(&path).ok();
    }
}
