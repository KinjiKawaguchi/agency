//! The real clock, the operating system's randomness, and an async write lock.
use agency_core::ports::{Clock, OutboxBell, Random, WriteLock};
use agency_proto::clock::{OffsetDateTime, UtcOffset};
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct SystemClock;

/// How long the machine's time zone is remembered before it is asked again (it changes with summer time).
const OFFSET_KEPT: Duration = Duration::from_secs(3600);

/// The offset `date +%z` says (`+0900`, `-0330`). The `time` crate refuses to read the local offset from a
/// program with several threads, so the machine is asked.
fn machine_offset() -> UtcOffset {
    let Ok(output) = std::process::Command::new("date").arg("+%z").output() else {
        return UtcOffset::UTC;
    };
    parse_offset(String::from_utf8_lossy(&output.stdout).trim()).unwrap_or(UtcOffset::UTC)
}

pub fn parse_offset(text: &str) -> Option<UtcOffset> {
    let sign = match text.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let digits = &text[1..];
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (hours, minutes): (i8, i8) = (digits[..2].parse().ok()?, digits[2..].parse().ok()?);
    UtcOffset::from_hms(sign * hours, sign * minutes, 0).ok()
}

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn utc_offset(&self) -> UtcOffset {
        static KEPT: Mutex<Option<(Instant, UtcOffset)>> = Mutex::new(None);
        let mut kept = KEPT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match *kept {
            Some((at, offset)) if at.elapsed() < OFFSET_KEPT => offset,
            _ => {
                let offset = machine_offset();
                *kept = Some((Instant::now(), offset));
                offset
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_operating_system_gives_fresh_random_bytes() {
        let (first, second) = (OsRandom.bytes16(), OsRandom.bytes16());
        assert_ne!(first, second, "two draws are never the same");
        assert_ne!(first, [0u8; 16], "a draw is not all zeros");
    }

    #[test]
    fn reads_the_offset_date_prints() {
        let hours = |h: i8, m: i8| UtcOffset::from_hms(h, m, 0).unwrap();
        assert_eq!(parse_offset("+0900"), Some(hours(9, 0)));
        assert_eq!(parse_offset("-0330"), Some(hours(-3, -30)));
        assert_eq!(parse_offset("+0000"), Some(UtcOffset::UTC));
        for bad in ["", "0900", "+09", "+09:00", "+09000", "+abcd", "UTC"] {
            assert_eq!(parse_offset(bad), None, "{bad:?}");
        }
    }
}

pub struct OsRandom;

impl Random for OsRandom {
    fn bytes16(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
        bytes
    }
}

#[derive(Default)]
pub struct TokioWriteLock(tokio::sync::Mutex<()>);

#[async_trait]
impl WriteLock for TokioWriteLock {
    async fn lock(&self) -> Box<dyn Send + '_> {
        Box::new(self.0.lock().await)
    }
}

/// Wakes the task that delivers the outbox when a parcel is queued. A ring while nothing waits is kept
/// for the next wait, so it is never lost.
#[derive(Default)]
pub struct OutboxWake(tokio::sync::Notify);

impl OutboxBell for OutboxWake {
    fn ring(&self) {
        self.0.notify_one();
    }
}

impl OutboxWake {
    /// Returns when the bell rings, or after `at_most`, whichever is first.
    pub async fn wait(&self, at_most: std::time::Duration) {
        let _ = tokio::time::timeout(at_most, self.0.notified()).await;
    }
}
