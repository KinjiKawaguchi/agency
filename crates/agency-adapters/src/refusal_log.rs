//! The node's log of federation requests it refused while showing the caller only "not found" (see
//! `agency_core::unseen`). The operator needs the real reason; a caller must not get it, and a flood of
//! junk requests must not fill the disk. So refusals that no member's valid signature vouches for are
//! logged at most `PER_MINUTE` times a minute, and a count of the rest follows. A refusal of a request
//! with a valid signature by a member (a peer whose clock is off, or one that asks a node its team has
//! left) is always logged: only members can make one.
use crate::Log;
use agency_core::error::Reason;
use std::sync::Mutex;
use std::time::Instant;

/// At most this many refusals without a member's valid signature are logged in a minute.
pub const PER_MINUTE: u32 = 20;

/// Counts the refusals of the current minute. Pure, so its rule is tested without a clock.
#[derive(Debug, Default)]
pub struct Limiter {
    minute: u64,
    logged: u32,
    dropped: u32,
}

impl Limiter {
    /// The lines to log for a refusal at `seconds` (since any fixed start), given its own `line`.
    pub fn lines(&mut self, seconds: u64, reason: &Reason, line: String) -> Vec<String> {
        if reason.is_from_member {
            return vec![line];
        }
        let mut lines = vec![];
        let minute = seconds / 60;
        if minute != self.minute {
            if self.dropped > 0 {
                lines.push(format!(
                    "agency: {} more refused federation requests were not logged",
                    self.dropped
                ));
            }
            *self = Self {
                minute,
                ..Self::default()
            };
        }
        if self.logged < PER_MINUTE {
            self.logged += 1;
            lines.push(line);
            if self.logged == PER_MINUTE {
                lines.push(
                    "agency: refused federation requests are not logged for the rest of this minute"
                        .into(),
                );
            }
        } else {
            self.dropped += 1;
        }
        lines
    }
}

pub struct RefusalLog {
    log: Log,
    limiter: Mutex<Limiter>,
    start: Instant,
}

impl RefusalLog {
    pub fn new(log: Log) -> Self {
        Self {
            log,
            limiter: Mutex::default(),
            start: Instant::now(),
        }
    }

    pub fn to_stderr() -> Self {
        Self::new(Box::new(|line| eprintln!("{line}")))
    }

    /// Logs a refused request with its reason. `path` must not carry a query.
    pub fn refused(&self, method: &str, path: &str, reason: &Reason) {
        let line = format!("agency: refused {method} {path}: {}", reason.text);
        let seconds = self.start.elapsed().as_secs();
        let lines = match self.limiter.lock() {
            Ok(mut limiter) => limiter.lines(seconds, reason, line),
            Err(_) => return,
        };
        for line in lines {
            (self.log)(&line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(is_from_member: bool) -> Reason {
        Reason {
            text: "x".into(),
            is_from_member,
        }
    }

    #[test]
    fn junk_is_logged_at_most_per_minute_and_a_members_refusal_always() {
        let mut limiter = Limiter::default();
        let mut logged = vec![];
        for n in 0..50 {
            logged.extend(limiter.lines(5, &reason(false), format!("junk {n}")));
        }
        assert_eq!(logged.len(), PER_MINUTE as usize + 1);
        assert_eq!(
            logged[PER_MINUTE as usize - 1],
            format!("junk {}", PER_MINUTE - 1)
        );
        assert!(logged[PER_MINUTE as usize].contains("not logged for the rest of this minute"));

        assert_eq!(
            limiter.lines(30, &reason(true), "a member".into()),
            ["a member"],
            "a member's refusal is logged even after the limit"
        );

        let next = limiter.lines(65, &reason(false), "junk next minute".into());
        assert_eq!(
            next,
            [
                format!(
                    "agency: {} more refused federation requests were not logged",
                    50 - PER_MINUTE
                ),
                "junk next minute".into()
            ]
        );
        let quiet = limiter.lines(130, &reason(false), "later".into());
        assert_eq!(quiet, ["later"], "nothing was dropped in the minute before");
    }
}
