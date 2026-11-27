//! The outbox. Everything sent to another node (decision envelopes, new org versions, and device changes) is queued here
//! first and removed once delivered. Unreachable nodes are retried with backoff; a parcel refused with a
//! 4xx (other than 408 and 429) is not retried and moves to the refused box.
use crate::error::Result;
use crate::node::Node;
use crate::ports::PeerError;
use crate::routes;
use agency_proto::clock::{self, Duration};
use agency_proto::digest::sha256_hex;
use agency_proto::org::SignedOrg;
use agency_proto::{DeviceChange, Envelope, Signed};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "doc", rename_all = "lowercase")]
pub enum ParcelBody {
    Envelope(Box<Signed<Envelope>>),
    Org(Box<SignedOrg>),
    /// For the org's root node.
    Device(Box<Signed<DeviceChange>>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcel {
    pub id: String,
    /// The receiving node's origin.
    pub to: String,
    pub body: ParcelBody,
    pub queued_at: String,
    pub attempts: u32,
    pub next_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Delivered,
    Refused(String),
    Unreachable(String),
}

/// Waits before the 1st, 2nd, ... retry; the last one repeats.
pub const BACKOFF_SECONDS: [i64; 5] = [15, 30, 60, 120, 300];

impl Parcel {
    /// An envelope keeps its id. An org version gets an id from its receiver, org, and version, so
    /// queuing the same version twice for the same node keeps one parcel.
    pub fn new(to: String, body: ParcelBody, now: &str) -> Self {
        let id = match &body {
            ParcelBody::Envelope(signed) => signed.body.id.as_str().to_owned(),
            ParcelBody::Org(doc) => {
                sha256_hex(format!("{to}\n{}\n{}", doc.body.org, doc.body.version).as_bytes())[..32]
                    .to_owned()
            }
            ParcelBody::Device(change) => sha256_hex(change.signature.as_bytes())[..32].to_owned(),
        };
        Self {
            id,
            to,
            body,
            queued_at: now.to_owned(),
            attempts: 0,
            next_at: now.to_owned(),
            last_error: None,
        }
    }
}

pub fn enqueue(node: &Node, parcel: &Parcel) -> Result<()> {
    node.store().put_parcel(parcel)
}

pub fn pending(node: &Node) -> Vec<Parcel> {
    node.store().pending_parcels()
}

/// 408, 429, and 5xx mean "try again later"; every other status is a refusal.
pub fn is_retryable(status: u16) -> bool {
    status == 408 || status == 429 || (500..600).contains(&status)
}

async fn send(node: &Node, parcel: &Parcel) -> Outcome {
    let sent = match &parcel.body {
        ParcelBody::Envelope(signed) => {
            node.peers()
                .post_envelope(
                    &parcel.to,
                    &routes::envelopes(&signed.body.org, &signed.body.team),
                    signed,
                )
                .await
        }
        ParcelBody::Org(doc) => {
            node.peers()
                .post_definition(&parcel.to, &routes::definition(&doc.body.org), doc)
                .await
        }
        ParcelBody::Device(change) => node
            .peers()
            .post_device_change(&parcel.to, &routes::devices(&change.body.org), change)
            .await
            .map(|_| ()),
    };
    outcome_of(sent)
}

pub(crate) fn outcome_of<T>(sent: Result<T, PeerError>) -> Outcome {
    match sent {
        Ok(_) => Outcome::Delivered,
        Err(PeerError::Status { status, message }) if !is_retryable(status) => {
            Outcome::Refused(message)
        }
        Err(error) => Outcome::Unreachable(error.to_string()),
    }
}

/// Delivers one parcel, then removes it, moves it to the refused box, or schedules its next attempt.
pub async fn deliver(node: &Node, parcel: &Parcel) -> Result<Outcome> {
    let outcome = send(node, parcel).await;
    settle(node, parcel, &outcome)?;
    Ok(outcome)
}

/// After an attempt: removes a delivered parcel, moves a refused one to the refused box, or schedules
/// the next attempt.
pub(crate) fn settle(node: &Node, parcel: &Parcel, outcome: &Outcome) -> Result<()> {
    // A node that answered, even to refuse, is there: the replicas of its teams that wait for it are read at the next pass.
    if !matches!(outcome, Outcome::Unreachable(_)) {
        node.note_answer(&parcel.to);
    }
    match outcome {
        Outcome::Delivered => node.store().remove_parcel(&parcel.id)?,
        Outcome::Refused(message) => {
            node.store().refuse_parcel(&Parcel {
                last_error: Some(message.clone()),
                ..parcel.clone()
            })?;
            node.store().remove_parcel(&parcel.id)?;
        }
        Outcome::Unreachable(message) => {
            let wait = BACKOFF_SECONDS[(parcel.attempts as usize).min(BACKOFF_SECONDS.len() - 1)];
            let next_at = clock::format(node.now() + Duration::seconds(wait));
            node.store().put_parcel(&Parcel {
                attempts: parcel.attempts + 1,
                next_at,
                last_error: Some(message.clone()),
                ..parcel.clone()
            })?;
        }
    }
    Ok(())
}

/// How many receiving nodes are delivered to at once. A node that does not answer holds one of these for its
/// timeout, not the whole outbox.
pub const NODES_AT_ONCE: usize = 8;

/// Delivers every parcel whose next attempt is due: to each receiving node in the order the parcels were queued
/// (so that versions of an org arrive in order), and to several nodes at once. Once a node is unreachable, its
/// other parcels wait for their next attempt instead of each waiting for the same timeout now; and a version that
/// is waiting for its next attempt holds back the later versions of its org for that node.
pub async fn deliver_due(node: &Node) -> Result<Vec<Outcome>> {
    use futures_util::stream::{self, StreamExt};
    let mut by_node: std::collections::BTreeMap<String, Vec<Parcel>> = Default::default();
    for parcel in pending(node) {
        by_node.entry(parcel.to.clone()).or_default().push(parcel);
    }
    let delivered: Vec<Result<Vec<Outcome>>> = stream::iter(by_node.into_values())
        .map(|parcels| deliver_in_order(node, parcels))
        .buffer_unordered(NODES_AT_ONCE)
        .collect()
        .await;
    let mut outcomes = vec![];
    for one in delivered {
        outcomes.extend(one?);
    }
    Ok(outcomes)
}

/// The org whose version a parcel carries, if it carries one.
fn org_of(parcel: &Parcel) -> Option<&agency_proto::Id> {
    match &parcel.body {
        ParcelBody::Org(doc) => Some(&doc.body.org),
        _ => None,
    }
}

/// The parcels for one node, oldest first: each that is due is delivered, until one finds the node unreachable.
/// A version that is not due yet holds back the later versions of its org, which the node could take only after it.
async fn deliver_in_order(node: &Node, mut parcels: Vec<Parcel>) -> Result<Vec<Outcome>> {
    // Two versions of an org queued in the same second go in version order: a node takes only the next one.
    let version = |parcel: &Parcel| match &parcel.body {
        ParcelBody::Org(doc) => doc.body.version,
        _ => 0,
    };
    parcels.sort_by(|one, other| {
        (&one.queued_at, version(one), &one.id).cmp(&(&other.queued_at, version(other), &other.id))
    });
    let now = node.now();
    let mut waiting_orgs: Vec<agency_proto::Id> = vec![];
    let mut outcomes = vec![];
    for parcel in &parcels {
        let org = org_of(parcel);
        if org.is_some_and(|org| waiting_orgs.contains(org)) {
            continue;
        }
        let is_due = clock::parse(&parcel.next_at).is_none_or(|at| at <= now);
        if !is_due {
            waiting_orgs.extend(org.cloned());
            continue;
        }
        let outcome = deliver(node, parcel).await?;
        let is_unreachable = matches!(outcome, Outcome::Unreachable(_));
        outcomes.push(outcome);
        if is_unreachable {
            break;
        }
    }
    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::is_retryable;

    #[test]
    fn only_timeouts_rate_limits_and_server_errors_are_retried() {
        for retried in [408, 429, 500, 502, 503, 599] {
            assert!(is_retryable(retried), "{retried}");
        }
        for refused in [400, 401, 403, 404, 409, 413, 600] {
            assert!(!is_retryable(refused), "{refused}");
        }
    }
}
