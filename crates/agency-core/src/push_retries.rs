//! A destination that did not take a notice is tried again, alone. A notice is recorded as told after its first
//! attempt (`notified`), so a browser whose push service had a bad minute is not forgotten because the phone took
//! the notice, and a destination that took it is never sent it twice. What is kept is the notice's key and the
//! destination, never its words: at a retry the notice is made again from what is true now, with the settings of
//! now (`hideTitles`), so no title is ever written to a file, and a notice about something that has passed (the
//! request was decided, the device request closed) is never sent late.
use crate::error::Result;
use crate::node::Node;
use crate::notice::{Notice, NoticeKind};
use crate::push::{remove_tokens, sent_of, Candidate, Pass, PushError};
use crate::web_push::{record_sends, remove_subscriptions, wanting};
use agency_proto::clock;
use serde::{Deserialize, Serialize};

/// How long a failed destination is tried again: a notice that is a day late is noise.
pub const RETRY_HOURS: i64 = 24;
/// The most retries kept; the oldest is dropped first.
pub const RETRIES_KEPT: usize = 1000;

/// Where a notice goes: an APNs token (as stored) or a browser's subscription (by its id).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Destination {
    Token(String),
    Browser(String),
}

/// A destination that is to be sent a notice again: the key of the notice (`org/team/request`, `device/<id>`,
/// `decision/<id>`) and when it first failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retry {
    pub key: String,
    pub destination: Destination,
    pub since: String,
}

/// What one notice sent again to one destination came to; `None` when the destination is no longer registered or no
/// longer wants this kind of notice, which ends the retry.
async fn send_again(
    node: &Node,
    destination: &Destination,
    notice: &Notice,
) -> Result<Option<std::result::Result<(), PushError>>> {
    match destination {
        Destination::Token(token) => {
            let Some(found) = node
                .store()
                .push_tokens()
                .into_iter()
                .find(|one| &one.token == token && notice.is_wanted_by(&one.events))
            else {
                return Ok(None);
            };
            let answer = node.notifier().send(&found, notice).await;
            if answer == Err(PushError::Gone) {
                remove_tokens(node, std::slice::from_ref(token))?;
            }
            Ok(Some(answer))
        }
        Destination::Browser(id) => {
            let Some(found) = wanting(node, notice).into_iter().find(|one| &one.id == id) else {
                return Ok(None);
            };
            let answer = node.notifier().send_web(&found, notice).await;
            if let Some(sent) = sent_of(&answer) {
                record_sends(node, &[(id.as_str(), sent)])?;
            }
            if answer == Err(PushError::Gone) {
                remove_subscriptions(node, std::slice::from_ref(id))?;
            }
            Ok(Some(answer))
        }
    }
}

fn is_expired(retry: &Retry, now: clock::OffsetDateTime) -> bool {
    clock::parse(&retry.since)
        .is_none_or(|since| now - since >= clock::Duration::hours(RETRY_HOURS))
}

/// What the retries of one pass came to: whether a request's alert reached an iPhone (which wakes the app, so the
/// desk needs no push of its own).
pub(crate) struct Retried {
    pub(crate) reached_iphone: bool,
}

/// Sends again to each destination that failed earlier, and keeps only the ones that failed again, with the ones
/// this pass added (`failed`). An entry ends when the destination took the notice or is gone, is no longer
/// registered or no longer wants the kind, the thing is no longer true (its key is not among the `candidates`),
/// it is a day old, or the cap pushes it out.
pub(crate) async fn retry_failed(
    node: &Node,
    candidates: &[Candidate],
    badge: u32,
    failed: Vec<Retry>,
    pass: &mut Pass,
) -> Result<Retried> {
    let now = node.now();
    let mut kept: Vec<Retry> = vec![];
    let mut reached_iphone = false;
    for retry in node.store().push_retries() {
        let Some(candidate) = candidates.iter().find(|one| one.key == retry.key) else {
            continue;
        };
        if is_expired(&retry, now) {
            continue;
        }
        let notice = Notice {
            badge,
            ..candidate.notice.clone()
        };
        match send_again(node, &retry.destination, &notice).await? {
            Some(Ok(())) => {
                pass.sent += 1;
                let is_request = matches!(notice.kind, NoticeKind::Request { .. });
                reached_iphone |= is_request && matches!(retry.destination, Destination::Token(_));
            }
            Some(Err(PushError::Failed(why))) => {
                pass.failures.push(why);
                kept.push(retry);
            }
            Some(Err(PushError::Gone)) | None => {}
        }
    }
    kept.extend(failed);
    node.store().save_push_retries(&newest(kept))?;
    Ok(Retried { reached_iphone })
}

/// A failure of one destination to take a fresh notice, to be tried again from the next pass.
pub fn failure_of(key: &str, destination: Destination, now: clock::OffsetDateTime) -> Retry {
    Retry {
        key: key.to_owned(),
        destination,
        since: clock::format(now),
    }
}

/// The newest `RETRIES_KEPT` retries, in order.
fn newest(mut retries: Vec<Retry>) -> Vec<Retry> {
    let start = retries.len().saturating_sub(RETRIES_KEPT);
    retries.drain(..start);
    retries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retry(n: usize) -> Retry {
        Retry {
            key: format!("acme/app/r-{n}"),
            destination: Destination::Browser(format!("b-{n}")),
            since: "2026-11-24T12:00:00Z".to_owned(),
        }
    }

    #[test]
    fn only_the_newest_thousand_are_kept() {
        let all: Vec<Retry> = (0..RETRIES_KEPT + 5).map(retry).collect();
        let kept = newest(all);
        assert_eq!(kept.len(), RETRIES_KEPT);
        assert_eq!(kept[0], retry(5), "the oldest go first");
        assert_eq!(kept.last(), Some(&retry(RETRIES_KEPT + 4)));
        assert_eq!(newest(vec![retry(1)]), vec![retry(1)]);
    }

    #[test]
    fn a_retry_is_a_key_a_destination_and_a_time_and_nothing_of_the_words() {
        let text = serde_json::to_string(&retry(1)).unwrap();
        assert_eq!(
            text,
            r#"{"key":"acme/app/r-1","destination":{"kind":"browser","id":"b-1"},"since":"2026-11-24T12:00:00Z"}"#
        );
        let again: Retry = serde_json::from_str(&text).unwrap();
        assert_eq!(again, retry(1));
        assert!(serde_json::from_str::<Retry>(r#"{"key":"k","destination":{"kind":"token","id":"t"},"since":"x","title":"Ship it?"}"#).is_err(), "no other field, a title least of all");
    }
}
