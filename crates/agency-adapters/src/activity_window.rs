//! One read of a team's activity, shared by the readers that ask for it within a short time. The command
//! is run once for a window that starts a little before the first reader's `since`, and each reader gets
//! the events after its own `since`, cut at its own `limit`, so readers with different questions still
//! start one process.
use agency_proto::activity::{Activity, ActivityEvent};
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::Timestamp;

/// What the runtime answered for events after `since`, at its largest limit.
#[derive(Clone, Debug)]
pub(crate) struct Window {
    pub since: OffsetDateTime,
    pub activity: Activity,
}

impl Window {
    /// The answer to a read of the events after `since`, at most `limit`, or `None` when this window cannot
    /// give it: it starts after `since`, or it was cut at the runtime's limit before it holds `limit`
    /// events after `since` (events past the cut are unknown, so only the runtime can say).
    pub fn answer(&self, since: &Timestamp, limit: u32) -> Option<Activity> {
        let wanted = clock::parse(since.as_str())?;
        if wanted < self.since {
            return None;
        }
        let mut after: Vec<&ActivityEvent> = vec![];
        for event in &self.activity.events {
            if clock::parse(event.at.as_str())? > wanted {
                after.push(event);
            }
        }
        let limit = limit as usize;
        if self.activity.truncated && after.len() < limit {
            return None;
        }
        Some(Activity {
            truncated: self.activity.truncated || after.len() > limit,
            events: after.into_iter().take(limit).cloned().collect(),
            ..self.activity.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../../testdata/activity.sample.json");

    fn window(truncated: bool) -> Window {
        let mut activity: Activity = serde_json::from_str(SAMPLE).unwrap();
        activity.truncated = truncated;
        Window {
            since: clock::parse("2026-11-24T11:00:00Z").unwrap(),
            activity,
        }
    }

    fn since(raw: &str) -> Timestamp {
        Timestamp::parse(raw).unwrap()
    }

    #[test]
    fn each_reader_gets_the_events_after_its_own_since() {
        let all = window(false)
            .answer(&since("2026-11-24T11:00:00Z"), 200)
            .unwrap();
        assert_eq!(all.events.len(), 6);
        let later = window(false)
            .answer(&since("2026-11-24T11:30:00Z"), 200)
            .unwrap();
        assert_eq!(later.events.len(), 2, "strictly after");
        assert!(!later.truncated);
    }

    #[test]
    fn a_smaller_limit_cuts_the_list_and_says_so() {
        let cut = window(false)
            .answer(&since("2026-11-24T11:00:00Z"), 2)
            .unwrap();
        assert_eq!((cut.events.len(), cut.truncated), (2, true));
    }

    #[test]
    fn a_window_that_starts_after_since_cannot_answer() {
        assert!(window(false)
            .answer(&since("2026-11-24T10:59:59Z"), 200)
            .is_none());
    }

    #[test]
    fn a_window_cut_by_the_runtime_answers_only_what_it_surely_holds() {
        let cut = window(true);
        assert!(
            cut.answer(&since("2026-11-24T11:00:00Z"), 200).is_none(),
            "more may follow"
        );
        let answer = cut.answer(&since("2026-11-24T11:00:00Z"), 3).unwrap();
        assert_eq!((answer.events.len(), answer.truncated), (3, true));
        let exact = cut.answer(&since("2026-11-24T11:25:00Z"), 3).unwrap();
        assert_eq!(
            (exact.events.len(), exact.truncated),
            (3, true),
            "the cut is passed on"
        );
        assert!(
            cut.answer(&since("2026-11-24T11:25:00Z"), 4).is_none(),
            "a fourth may be past the cut"
        );
    }
}
