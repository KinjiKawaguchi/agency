//! Push notifications for the owner (docs/protocol.md, "Push notifications"). The owner's app registers
//! its APNs device tokens and the kinds it wants; the node notifies each of them once when a request
//! starts waiting for the owner, a device asks to be added, or a decision of the owner's is refused.
//! Only titles and labels leave the node, and none when `push.hideTitles` is set.
use crate::error::{Error, Result};
use crate::node::Node;
pub use crate::notice::{Notice, NoticeKind};
use crate::push_events::{decision_candidates, device_candidates};
use crate::views::{mine, runtime_view, Reader};
use agency_proto::client::{
    default_push_events, PushEvent, PushToken, PushTokenInput, PushTokenList,
};
use agency_proto::view::Request;
use agency_proto::{Id, OrgDefinition, Timestamp};

/// The most tokens kept; the oldest is dropped first.
pub const MAX_TOKENS: usize = 20;
/// How many notified keys of each kind are remembered.
pub const NOTIFIED_KEPT: usize = 1000;
/// Marks a notified list that has seen device requests and refused decisions. A list without it is from
/// a node that only notified requests: the device requests and refusals it finds the first time are
/// recorded and not sent, so an upgrade does not announce the past.
pub(crate) const KINDS_MARKER: &str = "baseline/device-decision";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushError {
    /// The token is no longer valid; it is removed.
    Gone,
    /// Sending failed now; the request is tried again on the next pass.
    Failed(String),
}

/// A device token as hex, lowercased: an even number of 32 to 200 hex characters.
fn check_token(raw: &str) -> Result<String> {
    let token = raw.trim().to_ascii_lowercase();
    let is_hex = token.bytes().all(|b| b.is_ascii_hexdigit());
    if !is_hex || !(32..=200).contains(&token.len()) || !token.len().is_multiple_of(2) {
        return Err(Error::bad_request(
            "the token must be hex, 32 to 200 characters",
        ));
    }
    Ok(token)
}

/// The kinds a token gets: `request` only when it names none (absent), none for an empty list, each kind
/// once, in the order `request`, `device`, `decision`.
pub(crate) fn normal_events(events: Option<Vec<PushEvent>>) -> Vec<PushEvent> {
    let Some(mut events) = events else {
        return default_push_events();
    };
    events.sort();
    events.dedup();
    events
}

/// Registers a device token, or updates its environment, label, and kinds.
pub fn register_push_token(node: &Node, input: PushTokenInput) -> Result<PushToken> {
    let token = check_token(&input.token)?;
    let label = input.label.trim().to_owned();
    if !(1..=80).contains(&label.chars().count()) || label.chars().any(char::is_control) {
        return Err(Error::bad_request("the label is 1 to 80 characters"));
    }
    let registered_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let entry = PushToken {
        token,
        environment: input.environment,
        label,
        registered_at,
        events: normal_events(input.events),
    };
    let mut tokens: Vec<PushToken> = node
        .store()
        .push_tokens()
        .into_iter()
        .filter(|one| one.token != entry.token)
        .collect();
    tokens.push(entry.clone());
    let start = tokens.len().saturating_sub(MAX_TOKENS);
    node.store().save_push_tokens(&tokens[start..])?;
    Ok(entry)
}

/// Removes a device token. Removing an unknown token is not an error.
pub fn remove_push_token(node: &Node, raw: &str) -> Result<()> {
    let token = check_token(raw)?;
    remove_tokens(node, &[token])
}

fn remove_tokens(node: &Node, gone: &[String]) -> Result<()> {
    let kept: Vec<PushToken> = node
        .store()
        .push_tokens()
        .into_iter()
        .filter(|one| !gone.contains(&one.token))
        .collect();
    node.store().save_push_tokens(&kept)
}

/// The registered tokens, each shortened to its last 8 characters.
pub fn push_tokens(node: &Node) -> PushTokenList {
    let shorten = |one: PushToken| PushToken {
        token: one.token[one.token.len().saturating_sub(8)..].to_owned(),
        ..one
    };
    PushTokenList {
        tokens: node
            .store()
            .push_tokens()
            .into_iter()
            .map(shorten)
            .collect(),
    }
}

/// Something to notify about, and its key in the notified list.
pub(crate) struct Candidate {
    pub key: String,
    pub notice: Notice,
}

fn waiting(org: &Id, team: &Id, label: &str, request: &Request, is_hidden: bool) -> Candidate {
    let shown = |text: &str| (!is_hidden).then(|| text.to_owned());
    Candidate {
        key: format!("{org}/{team}/{}", request.id),
        notice: Notice {
            kind: NoticeKind::Request {
                org: org.clone(),
                team: team.clone(),
                request: request.id.clone(),
                team_label: shown(label),
                title: shown(&request.title),
            },
            badge: 0,
        },
    }
}

/// Requests that are the owner's to decide in one team, by the same rule as `TeamSnapshot.mine`.
async fn waiting_in(
    node: &Node,
    definition: &OrgDefinition,
    team: &Id,
    is_hidden: bool,
) -> Vec<Candidate> {
    let org = &definition.org;
    let Some(person) = definition.member(&node.account()) else {
        return vec![];
    };
    let (view, mine) = if let Some(hosted) = node.hosted(org, team) {
        let Ok(view) = runtime_view(node, &hosted).await else {
            return vec![];
        };
        let reader = Reader::Hosted {
            hosted: &hosted,
            person: &person.id,
        };
        let mine = mine(&view, &reader);
        (view, mine)
    } else {
        let replica = node.store().load_replica(org, team).unwrap_or_default();
        let Some(view) = replica.view else {
            return vec![];
        };
        let reader = Reader::Remote {
            viewer: replica.viewer.as_deref(),
        };
        let mine = mine(&view, &reader);
        (view, mine)
    };
    let requests = view.requests.iter();
    requests
        .filter(|request| mine.contains(&request.id))
        .map(|request| waiting(org, team, &view.label, request, is_hidden))
        .collect()
}

async fn waiting_for_owner(node: &Node, is_hidden: bool) -> Vec<Candidate> {
    let mut all = vec![];
    for doc in node.store().orgs() {
        for place in &doc.body.teams {
            all.extend(waiting_in(node, &doc.body, &place.id, is_hidden).await);
        }
    }
    all
}

/// What one pass sent, and the failures to report.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    pub sent: usize,
    pub failures: Vec<String>,
}

/// What sending one notice to one kind of destination came to.
#[derive(Default)]
struct Outcome {
    wanting: usize,
    accepted: bool,
    gone: Vec<String>,
}

impl Outcome {
    fn record(&mut self, id: &str, answer: Result<(), PushError>, pass: &mut Pass) {
        match answer {
            Ok(()) => self.accepted = true,
            Err(PushError::Gone) => self.gone.push(id.to_owned()),
            Err(PushError::Failed(why)) => pass.failures.push(why),
        }
    }
}

async fn send_to_tokens(node: &Node, notice: &Notice, pass: &mut Pass) -> Result<Outcome> {
    let all = node.store().push_tokens();
    let wanting: Vec<&PushToken> = all
        .iter()
        .filter(|token| token.events.contains(&notice.event()))
        .collect();
    let mut outcome = Outcome {
        wanting: wanting.len(),
        ..Outcome::default()
    };
    for token in wanting {
        let answer = node.notifier().send(token, notice).await;
        outcome.record(&token.token, answer, pass);
    }
    remove_tokens(node, &outcome.gone)?;
    Ok(outcome)
}

async fn send_to_browsers(node: &Node, notice: &Notice, pass: &mut Pass) -> Result<Outcome> {
    let wanting = crate::web_push::wanting(node, notice);
    let mut outcome = Outcome {
        wanting: wanting.len(),
        ..Outcome::default()
    };
    for subscription in &wanting {
        let answer = node.notifier().send_web(subscription, notice).await;
        outcome.record(&subscription.id, answer, pass);
    }
    crate::web_push::remove_subscriptions(node, &outcome.gone)?;
    Ok(outcome)
}

/// Sends one notice to every token and browser subscription that wants its kind. It counts as delivered
/// if any of them took it, or if none wants it (including when none is registered), or if the ones that
/// wanted it are gone; otherwise it is tried again on the next pass. Tokens APNs no longer accepts, and
/// subscriptions a push service says are gone, are removed.
async fn send_to_all(node: &Node, notice: &Notice, pass: &mut Pass) -> Result<bool> {
    let tokens = send_to_tokens(node, notice, pass).await?;
    let browsers = send_to_browsers(node, notice, pass).await?;
    let wanting = tokens.wanting + browsers.wanting;
    let gone = tokens.gone.len() + browsers.gone.len();
    Ok(wanting == 0 || tokens.accepted || browsers.accepted || gone == wanting)
}

/// Whether the node leaves titles and labels out of every notification: one meaning for APNs and Web
/// Push alike, since both go through a service that is not the node's own.
pub(crate) fn is_hiding_titles(node: &Node) -> bool {
    let config = node.config();
    config.hide_titles || config.push.is_some_and(|push| push.hide_titles)
}

/// The kind a notified key belongs to, which decides how many of them are kept.
fn kind_of_key(key: &str) -> u8 {
    match key {
        KINDS_MARKER => 3,
        _ if key.starts_with("device/") => 1,
        _ if key.starts_with("decision/") => 2,
        _ => 0,
    }
}

/// The newest `NOTIFIED_KEPT` keys of each kind, in order, and the marker, so a burst of one kind never
/// pushes out the keys of another (a request still waiting would be announced again).
fn trimmed(keys: Vec<String>) -> Vec<String> {
    let mut seen = [0usize; 4];
    let mut kept: Vec<String> = keys
        .into_iter()
        .rev()
        .filter(|key| {
            let kind = usize::from(kind_of_key(key));
            seen[kind] += 1;
            kind == 3 || seen[kind] <= NOTIFIED_KEPT
        })
        .collect();
    kept.reverse();
    kept
}

/// Notifies the owner of whatever started waiting since the last pass: requests, device requests, and
/// refused decisions. The first pass of a node with an empty data directory only records what is there,
/// so nothing old is sent.
pub async fn notify_waiting(node: &Node) -> Result<Pass> {
    let is_hidden = is_hiding_titles(node);
    let requests = waiting_for_owner(node, is_hidden).await;
    let badge = u32::try_from(requests.len()).unwrap_or(u32::MAX);
    let mut others = device_candidates(node, is_hidden);
    others.extend(decision_candidates(node, is_hidden));
    let Some(mut notified) = node.store().notified() else {
        let mut keys: Vec<String> = requests.into_iter().map(|one| one.key).collect();
        keys.extend(others.into_iter().map(|one| one.key));
        keys.push(KINDS_MARKER.to_owned());
        node.store().save_notified(&trimmed(keys))?;
        return Ok(Pass::default());
    };
    let is_upgrade = !notified.iter().any(|key| key == KINDS_MARKER);
    let mut pass = Pass::default();
    let mut candidates = requests;
    if is_upgrade {
        notified.extend(others.into_iter().map(|one| one.key));
        notified.push(KINDS_MARKER.to_owned());
    } else {
        candidates.extend(others);
    }
    let fresh: Vec<Candidate> = candidates
        .into_iter()
        .filter(|one| !notified.contains(&one.key))
        .collect();
    for one in fresh {
        let notice = Notice {
            badge,
            ..one.notice
        };
        if send_to_all(node, &notice, &mut pass).await? {
            pass.sent += 1;
            notified.push(one.key);
        }
    }
    node.store().save_notified(&trimmed(notified))?;
    Ok(pass)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_of_one_kind_does_not_push_out_the_keys_of_another() {
        let mut keys = vec!["acme/app/r-1".to_owned(), KINDS_MARKER.to_owned()];
        keys.extend((0..NOTIFIED_KEPT + 5).map(|n| format!("decision/{n}")));
        keys.push("device/d-1".to_owned());
        let kept = trimmed(keys);
        assert!(
            kept.contains(&"acme/app/r-1".to_owned()),
            "a request still waiting"
        );
        assert!(kept.contains(&KINDS_MARKER.to_owned()) && kept.contains(&"device/d-1".to_owned()));
        let decisions: Vec<&String> = kept
            .iter()
            .filter(|key| key.starts_with("decision/"))
            .collect();
        assert_eq!(decisions.len(), NOTIFIED_KEPT);
        assert_eq!(
            decisions[0], "decision/5",
            "the oldest of the kind go first"
        );
    }
}
