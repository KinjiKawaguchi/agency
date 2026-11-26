//! When to read a source's usage again, and why a read failed.
use std::fmt;
use std::time::Duration;

/// The time between two reads of one source while all goes well: usage moves slowly, and the
/// endpoint is not meant to be polled often.
pub const INTERVAL: Duration = Duration::from_secs(300);
/// The longest wait after failures.
pub const LONGEST_WAIT: Duration = Duration::from_secs(3600);

/// Why a read failed. The text of a failure never holds a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No login could be found where the source says it lives.
    NoLogin(String),
    /// The login's access token has run out. Only the program the login belongs to refreshes it:
    /// a refresh by the node would replace the refresh token that program holds.
    Expired,
    /// The token has expired for long enough that the node ran `claude` to renew the login, and that
    /// failed; the text says why.
    NotRenewed(String),
    /// The service refused the token.
    Rejected,
    /// The service asked to be called less often, and perhaps said for how long.
    RateLimited(Option<Duration>),
    /// The service could not be reached, or answered with an error.
    Unavailable(String),
    /// The answer is not what the service is known to answer.
    Malformed,
    /// The login has no plan windows to report (a login with an API key has none).
    NoWindows,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLogin(why) => write!(f, "no login found ({why})"),
            Self::Expired => write!(
                f,
                "the login's access token has expired; it renews when Claude Code next runs with it"
            ),
            Self::NotRenewed(why) => write!(
                f,
                "the login's access token has expired and could not be renewed: {why}"
            ),
            Self::Rejected => write!(f, "the service refused the login's token"),
            Self::RateLimited(_) => write!(f, "the service asked to be called less often"),
            Self::Unavailable(why) => write!(f, "the service did not answer ({why})"),
            Self::Malformed => write!(f, "the answer is not a usage report"),
            Self::NoWindows => write!(
                f,
                "the login has no plan windows (a login with an API key has none)"
            ),
        }
    }
}

/// The wait before the next read, after `consecutive` failures in a row (at least 1): the interval,
/// doubled with each further failure, up to the longest wait; at least as long as the service asked.
pub fn wait_after_failure(consecutive: u32, failure: &Failure) -> Duration {
    let doublings = consecutive.saturating_sub(1).min(6);
    let backoff = INTERVAL.saturating_mul(1 << doublings).min(LONGEST_WAIT);
    match failure {
        Failure::RateLimited(Some(asked)) => backoff.max(*asked).min(LONGEST_WAIT),
        _ => backoff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_longer_after_each_failure_up_to_the_longest_wait() {
        let failure = Failure::Malformed;
        let waits: Vec<u64> = (1..=6)
            .map(|n| wait_after_failure(n, &failure).as_secs())
            .collect();
        assert_eq!(waits, [300, 600, 1200, 2400, 3600, 3600]);
        assert_eq!(wait_after_failure(u32::MAX, &failure), LONGEST_WAIT);
    }

    #[test]
    fn keeps_to_what_the_service_asked_for() {
        let asked = |seconds| Failure::RateLimited(Some(Duration::from_secs(seconds)));
        assert_eq!(wait_after_failure(1, &asked(1800)).as_secs(), 1800);
        assert_eq!(
            wait_after_failure(1, &asked(10)),
            INTERVAL,
            "never sooner than the interval"
        );
        assert_eq!(wait_after_failure(1, &asked(86_400)), LONGEST_WAIT);
    }
}
