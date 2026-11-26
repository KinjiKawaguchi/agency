//! A team's recent activity: what moved on its board, which pull requests opened and merged, and which
//! participant messaged which (docs/protocol.md, "Activity"). Board events and message edges only: a
//! message is told by its sender, its recipient, and the short summary the sender gave, never by its body.
//! An optional field is left out when absent, never written as `null`.
use crate::clock;
use crate::ids::Id;
use crate::timestamp::Timestamp;
use crate::view::{limits, WorkStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// The most events one answer may hold, and what a read asks for without `limit`.
pub const MAX_LIMIT: u32 = 500;
pub const DEFAULT_LIMIT: u32 = 200;
/// Without `since`, a read asks for the last hour.
pub const DEFAULT_SINCE_MINUTES: i64 = 60;
/// A message's summary, in characters.
pub const SUMMARY: usize = 120;
/// An event's id: 1 to this many characters of `a-z`, `0-9`, and `-`.
const ID_LENGTH: usize = 64;

/// Whether `limit` is a number of events a read may ask for.
pub fn valid_limit(limit: u32) -> bool {
    (1..=MAX_LIMIT).contains(&limit)
}

/// One thing that happened, at the time it happened (a status change: when the runtime first saw it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityEvent {
    /// Short, stable, and unique in the answer, so a client that asks again does not show an event twice.
    pub id: String,
    pub at: Timestamp,
    #[serde(flatten)]
    #[ts(flatten)]
    pub happened: Happened,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, optional_fields)]
pub enum Happened {
    /// A work item appeared on the board. `ref` is its item reference.
    ItemAdded {
        #[serde(rename = "ref")]
        #[ts(rename = "ref")]
        reference: String,
        title: String,
        /// Who it is assigned to, when the runtime knows it at the time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assignee: Option<String>,
        /// The item reference of the parent epic, when the item has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epic: Option<String>,
    },
    /// A work item moved to another status.
    StatusChanged {
        #[serde(rename = "ref")]
        #[ts(rename = "ref")]
        reference: String,
        title: String,
        from: WorkStatus,
        to: WorkStatus,
        /// The item reference of the parent epic, when the item has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epic: Option<String>,
    },
    /// A pull request opened. `by` is a participant's name, only when the author is one; `issue` is the
    /// item reference of the issue it is for, when known.
    PrOpened {
        #[serde(rename = "ref")]
        #[ts(rename = "ref")]
        reference: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        issue: Option<String>,
        /// The item reference of the parent epic, when the item has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epic: Option<String>,
    },
    /// A pull request merged. `lead_seconds` is the time from opened to merged, when both are known.
    PrMerged {
        #[serde(rename = "ref")]
        #[ts(rename = "ref")]
        reference: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        issue: Option<String>,
        /// The item reference of the parent epic, when the item has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epic: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(type = "number")]
        lead_seconds: Option<u64>,
    },
    /// A participant messaged another. `to` is a plain name that need not be a participant of this team;
    /// `summary` is what the sender gave (empty when nothing), never taken from the message.
    Message {
        from: String,
        to: String,
        summary: String,
    },
}

/// `GET /api/orgs/<org>/teams/<team>/activity`: the events after `since`, oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Activity {
    pub org: Id,
    pub team: Id,
    pub generated_at: Timestamp,
    pub events: Vec<ActivityEvent>,
    /// `limit` cut the list: there are events after the last one, which a read with `since` set to its
    /// `at` returns.
    pub truncated: bool,
}

fn within(what: &str, text: &str, limit: usize) -> Result<(), String> {
    if text.chars().count() > limit {
        return Err(format!("{what} is longer than {limit} characters"));
    }
    Ok(())
}

fn is_event_id(id: &str) -> bool {
    (1..=ID_LENGTH).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Happened {
    /// Every text of the event within its limit.
    fn check_limits(&self) -> Result<(), String> {
        let names = |texts: &[Option<&String>]| {
            texts
                .iter()
                .flatten()
                .try_for_each(|text| within("a name", text, limits::NAME))
        };
        match self {
            Self::ItemAdded {
                reference,
                title,
                assignee,
                epic,
            } => names(&[
                Some(reference),
                Some(title),
                assignee.as_ref(),
                epic.as_ref(),
            ]),
            Self::StatusChanged {
                reference,
                title,
                epic,
                ..
            } => names(&[Some(reference), Some(title), epic.as_ref()]),
            Self::PrOpened {
                reference,
                title,
                by,
                issue,
                epic,
            }
            | Self::PrMerged {
                reference,
                title,
                by,
                issue,
                epic,
                ..
            } => names(&[
                Some(reference),
                Some(title),
                by.as_ref(),
                issue.as_ref(),
                epic.as_ref(),
            ]),
            Self::Message { from, to, summary } => {
                names(&[Some(from), Some(to)])?;
                within("a summary", summary, SUMMARY)
            }
        }
    }
}

impl Activity {
    /// Checks that this is a well-formed answer to a read of `org` and `team` for the events after `since`,
    /// at most `limit` of them: the events oldest first (equal times keep the runtime's order), each after
    /// `since`, their ids valid and unique, and every text within its limit.
    pub fn check(&self, org: &Id, team: &Id, since: &Timestamp, limit: u32) -> Result<(), String> {
        if &self.org != org || &self.team != team {
            return Err(format!(
                "reported {}/{} instead of {org}/{team}",
                self.org, self.team
            ));
        }
        if self.events.len() > limit as usize {
            return Err(format!("reported more than {limit} events"));
        }
        let since = clock::parse(since.as_str()).ok_or("a malformed since")?;
        let mut previous = since;
        let mut ids = HashSet::new();
        for event in &self.events {
            let at = clock::parse(event.at.as_str()).ok_or("an event has a malformed time")?;
            if at <= since {
                return Err("reported an event that is not after since".into());
            }
            if at < previous {
                return Err("reported events out of order".into());
            }
            previous = at;
            if !is_event_id(&event.id) {
                return Err("an event has a malformed id".into());
            }
            if !ids.insert(event.id.as_str()) {
                return Err("reported an event twice".into());
            }
            event.happened.check_limits()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const SAMPLE: &str = include_str!("../../../testdata/activity.sample.json");

    fn sample() -> Activity {
        serde_json::from_str(SAMPLE).unwrap()
    }

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn since() -> Timestamp {
        Timestamp::parse("2026-11-24T11:00:00Z").unwrap()
    }

    fn check(activity: &Activity) -> Result<(), String> {
        activity.check(&id("acme"), &id("app"), &since(), DEFAULT_LIMIT)
    }

    #[test]
    fn the_sample_round_trips_and_holds_every_kind() {
        let again = serde_json::to_value(sample()).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        assert!(!SAMPLE.contains("null"), "an optional field is left out");
        let kinds: Vec<&str> = again["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["kind"].as_str().unwrap())
            .collect();
        for kind in [
            "itemAdded",
            "statusChanged",
            "prOpened",
            "prMerged",
            "message",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
        assert_eq!(check(&sample()), Ok(()));
    }

    #[test]
    fn an_event_of_an_unknown_kind_or_with_a_malformed_time_does_not_parse() {
        let mut value: Value = serde_json::from_str(SAMPLE).unwrap();
        value["events"][0]["kind"] = json!("teleported");
        assert!(serde_json::from_value::<Activity>(value).is_err());
        let mut value: Value = serde_json::from_str(SAMPLE).unwrap();
        value["events"][0]["at"] = json!("2026-11-24T11:05:00+09:00");
        assert!(serde_json::from_value::<Activity>(value).is_err());
    }

    #[test]
    fn a_malformed_answer_is_refused() {
        let refused = |change: &dyn Fn(&mut Activity)| {
            let mut activity = sample();
            change(&mut activity);
            check(&activity).unwrap_err()
        };
        assert!(refused(&|a| a.team = id("other")).contains("instead of"));
        assert!(refused(&|a| a.events.reverse()).contains("out of order"));
        assert!(refused(&|a| a.events[0].at = since()).contains("not after since"));
        assert!(refused(&|a| a.events[1].id = a.events[0].id.clone()).contains("twice"));
        assert!(refused(&|a| a.events[0].id = "B-1".into()).contains("malformed id"));
        assert!(refused(&|a| a.events[0].id = String::new()).contains("malformed id"));
        let long = "x".repeat(SUMMARY + 1);
        assert!(refused(&|a| {
            a.events[2].happened = Happened::Message {
                from: "a".into(),
                to: "b".into(),
                summary: long.clone(),
            }
        })
        .contains("summary"));
        let title = "t".repeat(limits::NAME + 1);
        assert!(refused(&|a| {
            a.events[0].happened = Happened::ItemAdded {
                reference: "app#1".into(),
                title: title.clone(),
                assignee: None,
                epic: None,
            }
        })
        .contains("longer than"));
        assert!(sample()
            .check(&id("acme"), &id("app"), &since(), 3)
            .is_err());
    }

    #[test]
    fn events_that_share_a_time_keep_their_order() {
        let mut activity = sample();
        activity.events[1].at = activity.events[0].at.clone();
        assert_eq!(check(&activity), Ok(()));
        assert!(valid_limit(1) && valid_limit(MAX_LIMIT));
        assert!(!valid_limit(0) && !valid_limit(MAX_LIMIT + 1));
    }
}
