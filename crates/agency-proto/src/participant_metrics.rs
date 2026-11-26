//! What a team runtime knows about each participant's days and which pull requests were theirs
//! (`participantMetrics` in agency-runtime/1): the facts GitHub does not have. The times of the pull
//! requests and the issues are GitHub's, and the node reads them there (docs/protocol.md, "Evaluation");
//! this report only says whose work it was, and how the participant's time and tokens went.
use crate::ids::Id;
use crate::timestamp::{Date, Timestamp};
use crate::usage::{valid_days, TokenCounts};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The offsets from UTC that a time zone has had, in minutes east: UTC-12:00 to UTC+14:00.
pub const MIN_UTC_OFFSET_MINUTES: i32 = -720;
pub const MAX_UTC_OFFSET_MINUTES: i32 = 840;
/// The longest a calendar day lasts, in seconds: 25 hours, when the clocks go back.
pub const MAX_DAY_SECONDS: u32 = 25 * 3600;
/// The longest a participant's name or a pull request's repository may be, in bytes.
pub const MAX_NAME_BYTES: usize = 200;
/// How many participants and how many pull requests a report may list.
pub const MAX_PARTICIPANTS: usize = 1000;
pub const MAX_PULL_REQUESTS: usize = 10_000;

/// Whether `minutes` is the offset of some time zone.
pub fn valid_utc_offset(minutes: i32) -> bool {
    (MIN_UTC_OFFSET_MINUTES..=MAX_UTC_OFFSET_MINUTES).contains(&minutes)
}

/// A team's participants over the last `days` calendar days, today included, cut at `utcOffsetMinutes`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParticipantMetrics {
    pub org: Id,
    pub team: Id,
    pub days: u32,
    pub generated_at: Timestamp,
    /// The time zone that cut the days, in minutes east of UTC: the one the node asked for, or, when it
    /// asked for none, the runtime's own.
    pub utc_offset_minutes: i32,
    /// The participants that had any time, cost, or tokens in the range, in any order.
    pub participants: Vec<ParticipantDays>,
    /// The pull requests whose author is known: only those, never a guess.
    pub pull_requests: Vec<PullRequestAuthor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParticipantDays {
    /// The participant's name in the team runtime, as in `TeamView` and `TokenUsage`.
    pub name: String,
    /// Oldest day first; days with nothing in them may be left out.
    pub by_day: Vec<ParticipantDay>,
}

/// One participant's day. The three times add up to the time at least one of the participant's sessions
/// was alive, so when several sessions overlap, the overlap counts once; any time with no session alive
/// is in none of them. A moment that is both busy and waiting is busy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParticipantDay {
    pub date: Date,
    /// At least one session was working.
    pub busy_seconds: u32,
    /// No session was working and at least one waited for a decision of a human.
    pub waiting_seconds: u32,
    /// Sessions were alive and none worked or waited.
    pub idle_seconds: u32,
    /// Estimated cost in USD at API prices, as in `TokenUsage`; it adds up over overlapping sessions.
    pub cost_usd: f64,
    pub tokens: TokenCounts,
}

/// A pull request and the participant whose session made it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PullRequestAuthor {
    /// `owner/name` on GitHub.
    pub repo: String,
    #[ts(type = "number")]
    pub number: u64,
    /// The participant's name in the team runtime.
    pub by: String,
}

fn valid_repo(repo: &str) -> bool {
    let part = |text: &str| {
        !text.is_empty()
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    repo.len() <= MAX_NAME_BYTES
        && repo
            .split_once('/')
            .is_some_and(|(owner, name)| part(owner) && part(name))
}

impl ParticipantMetrics {
    /// Checks that this is a well-formed report of the team and range that were asked for, cut at the
    /// offset that was asked for, if any.
    pub fn check(
        &self,
        org: &Id,
        team: &Id,
        days: u32,
        asked_offset: Option<i32>,
    ) -> Result<(), String> {
        if &self.org != org || &self.team != team || self.days != days || !valid_days(days) {
            return Err(format!(
                "reported participants of {}/{} over {} days instead of {org}/{team} over {days} days",
                self.org, self.team, self.days
            ));
        }
        if !valid_utc_offset(self.utc_offset_minutes) {
            return Err("reported a time zone that does not exist".into());
        }
        if asked_offset.is_some_and(|asked| asked != self.utc_offset_minutes) {
            return Err("cut the days at another time zone than the one asked for".into());
        }
        if self.participants.len() > MAX_PARTICIPANTS
            || self.pull_requests.len() > MAX_PULL_REQUESTS
        {
            return Err(
                "reported more participants or pull requests than a report may hold".into(),
            );
        }
        self.check_participants(days)?;
        self.check_pull_requests()
    }

    fn check_participants(&self, days: u32) -> Result<(), String> {
        let mut names = std::collections::BTreeSet::new();
        for participant in &self.participants {
            let name = &participant.name;
            if name.is_empty() || name.len() > MAX_NAME_BYTES {
                return Err("reported a participant without a usable name".into());
            }
            if !names.insert(name.as_str()) {
                return Err(format!("reported {name} twice"));
            }
            if participant.by_day.len() > days as usize {
                return Err(format!("reported more than {days} days of {name}"));
            }
            let ascending = participant
                .by_day
                .windows(2)
                .all(|pair| pair[0].date.as_str() < pair[1].date.as_str());
            if !ascending {
                return Err(format!("reported the days of {name} out of order"));
            }
            for day in &participant.by_day {
                check_day(name, day)?;
            }
        }
        Ok(())
    }

    fn check_pull_requests(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for pull_request in &self.pull_requests {
            let PullRequestAuthor { repo, number, by } = pull_request;
            if !valid_repo(repo) || *number == 0 {
                return Err("reported a pull request that is not owner/name and a number".into());
            }
            if by.is_empty() || by.len() > MAX_NAME_BYTES {
                return Err(format!("reported {repo}#{number} without a usable author"));
            }
            if !seen.insert((repo.as_str(), *number)) {
                return Err(format!("reported {repo}#{number} twice"));
            }
        }
        Ok(())
    }
}

fn check_day(name: &str, day: &ParticipantDay) -> Result<(), String> {
    let seconds =
        u64::from(day.busy_seconds) + u64::from(day.waiting_seconds) + u64::from(day.idle_seconds);
    if seconds > u64::from(MAX_DAY_SECONDS) {
        return Err(format!(
            "reported more time than a day holds for {name} on {}",
            day.date.as_str()
        ));
    }
    if !(day.cost_usd.is_finite() && day.cost_usd >= 0.0) {
        return Err(format!("reported a malformed cost for {name}"));
    }
    Ok(())
}
