//! Decisions (kind `envelope`). A person's decision is signed with one of their devices; no node signs one.
//! The team's node verifies it before applying it to the team runtime.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::outbox::{deliver, enqueue, Outcome, Parcel, ParcelBody};
use crate::unseen;
use agency_proto::device::envelope_key;
use agency_proto::runtime::RuntimeError;
use agency_proto::Kind;
use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Id, Payload, RuntimeDecision, Signed};
use serde::{Deserialize, Serialize};

/// A decision this node's account made, as kept in the decision log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRecord {
    pub id: EnvelopeId,
    pub org: Id,
    pub team: Id,
    pub kind: EnvelopeKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice_label: Option<String>,
    pub payload: Payload,
    pub at: String,
    /// Applied to a team on this node; otherwise sent as an envelope, whose delivery the outbox tracks.
    pub is_local: bool,
}

/// What people read about a decision once its request is gone: the request's title and the label of the
/// chosen option. Kept in the decision log only; never signed or sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionNote {
    pub title: Option<String>,
    pub choice_label: Option<String>,
}

impl DecisionRecord {
    pub(crate) fn of(envelope: &Envelope, note: DecisionNote, is_local: bool) -> Self {
        Self {
            id: envelope.id.clone(),
            org: envelope.org.clone(),
            team: envelope.team.clone(),
            kind: envelope.kind,
            target: envelope.target.clone(),
            title: note.title,
            choice_label: note.choice_label,
            payload: envelope.payload.clone(),
            at: envelope.at.clone(),
            is_local,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decided {
    /// Applied to a team on this node.
    Applied,
    /// Delivered to the team's node.
    Delivered,
    /// The team's node could not be reached, so the envelope waits in the outbox.
    Queued(String),
    Refused(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Accepted {
    Applied,
    /// The same envelope arrived before (a retried delivery); it is not applied again.
    Duplicate,
}

/// A runtime's 400 and 404 are refusals; everything else means the runtime cannot act now.
fn runtime_refusal(error: RuntimeError) -> Error {
    let kind = match error.status {
        400 => ErrorKind::BadRequest,
        404 => ErrorKind::NotFound,
        _ => ErrorKind::Unavailable,
    };
    Error::new(kind, error.message)
}

/// Queues a signed envelope for the team's node, logs it, and tries to deliver it now.
pub(crate) async fn send_envelope(
    node: &Node,
    to: String,
    signed: Signed<Envelope>,
    record: DecisionRecord,
) -> Result<Decided> {
    let parcel = Parcel::new(to, ParcelBody::Envelope(Box::new(signed)), &node.now_text());
    enqueue(node, &parcel)?;
    node.store().append_decision(&record)?;
    Ok(match deliver(node, &parcel).await? {
        Outcome::Delivered => Decided::Delivered,
        Outcome::Unreachable(message) => Decided::Queued(message),
        Outcome::Refused(message) => Decided::Refused(message),
    })
}

/// The member who signed an envelope, with one of their devices; an account key never signs one. A
/// non-member is not found, so a stranger cannot tell what exists.
/// Verifies an envelope sent to a team on this node and applies it. Checked in order: shape, addressee,
/// membership and signature, expiry, and replay. An unknown org or team, a non-member, an unknown
/// device, and a wrong signature are all refused like an unknown team (see `unseen`).
pub async fn accept_envelope(
    node: &Node,
    org: &Id,
    team: &Id,
    signed: Signed<Envelope>,
) -> Result<Accepted> {
    let envelope = &signed.body;
    envelope.validate().map_err(Error::bad_request)?;
    if &envelope.org != org || &envelope.team != team {
        return Err(Error::bad_request(
            "the envelope's org and team differ from where it was sent",
        ));
    }
    let hosted = node.config().hosted(org, team).cloned();
    let doc = node.store().load_org(org).filter(|_| hosted.is_some());
    let found = doc
        .as_ref()
        .and_then(|doc| envelope_key(&doc.body, &signed));
    unseen::verify(&signed, Kind::Envelope, found.map(|(_, key)| key))?;
    let (Some(hosted), Some((person, _))) = (hosted, found) else {
        return Err(unseen::not_found());
    };
    if !envelope.is_fresh(node.now()) {
        return Err(Error::bad_request(
            "the envelope expired, or its time is too far ahead",
        ));
    }
    let _guard = node.write_lock().lock().await;
    if node.store().is_applied(&envelope.id) {
        return Ok(Accepted::Duplicate);
    }
    let decision = RuntimeDecision {
        kind: envelope.kind,
        target: envelope.target.clone(),
        by: person.id.clone(),
        payload: envelope.payload.clone(),
    };
    node.runtime()
        .decide(&hosted, decision)
        .await
        .map_err(runtime_refusal)?;
    node.store()
        .record_applied(&envelope.id, &node.now_text())?;
    Ok(Accepted::Applied)
}
