//! The outbox. Everything sent to another node (decision envelopes and new org versions) is queued here
//! first and removed once delivered. Unreachable nodes are retried with backoff; a parcel refused with a
//! 4xx (other than 408 and 429) is not retried and moves to the refused box.
use crate::error::Result;
use crate::node::Node;
use crate::ports::PeerError;
use crate::routes;
use agency_proto::clock::{self, Duration};
use agency_proto::digest::sha256_hex;
use agency_proto::org::SignedOrg;
use agency_proto::{Envelope, Signed};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "doc", rename_all = "lowercase")]
pub enum ParcelBody {
    Envelope(Box<Signed<Envelope>>),
    Org(Box<SignedOrg>),
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
    };
    match sent {
        Ok(()) => Outcome::Delivered,
        Err(PeerError::Status { status, message }) if !is_retryable(status) => {
            Outcome::Refused(message)
        }
        Err(error) => Outcome::Unreachable(error.to_string()),
    }
}

/// Delivers one parcel, then removes it, moves it to the refused box, or schedules its next attempt.
pub async fn deliver(node: &Node, parcel: &Parcel) -> Result<Outcome> {
    let outcome = send(node, parcel).await;
    match &outcome {
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
    Ok(outcome)
}

/// Delivers, oldest first, every parcel whose next attempt is due.
pub async fn deliver_due(node: &Node) -> Result<Vec<Outcome>> {
    let now = node.now();
    let mut outcomes = vec![];
    for parcel in pending(node)
        .into_iter()
        .filter(|one| clock::parse(&one.next_at).is_none_or(|at| at <= now))
    {
        outcomes.push(deliver(node, &parcel).await?);
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
