//! Keeping a client's decision. The node checks the envelope, puts it in its outbox and the decision log,
//! and answers at once; a background task then applies it to the team's runtime (a team on this node) or
//! posts it to the team's node, with retries (see `outbox`). The answer never waits for a runtime or a
//! peer, which can take 20 seconds or more. Sending the same envelope again is safe: it is kept once, and
//! the answer says where it is.
use crate::decisions::{accept_envelope_for, DecisionNote, DecisionRecord};
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::outbox::{Outcome, Parcel, ParcelBody};
use crate::signed_decisions::{check_signed, remote_relayed_for};
use agency_proto::client::RelayedFor;
use agency_proto::{Envelope, EnvelopeId, Id, Signed};

/// Where a decision is when the node answers the client.
#[derive(Debug, PartialEq, Eq)]
pub enum Kept {
    /// In the outbox, not yet taken by the team's runtime or node. `Some` says why the last try failed.
    Queued(Option<String>),
    /// Applied to a team on this node.
    Applied,
    /// Delivered to the team's node.
    Delivered,
    /// The team's runtime or node refused it; it is not retried.
    Refused(String),
}

fn could_not_keep() -> Error {
    Error::new(
        ErrorKind::Unavailable,
        "this node could not keep the decision now; send it again",
    )
}

/// Keeps a decision signed by one of the devices of the account the caller acts as, after the checks that need no runtime
/// and no peer: the account, the org and team, the shape, the freshness, and the device's signature. That
/// the decider is the request's addressee needs the team's runtime, so the background task checks it and a
/// refusal shows in the decision log.
pub fn keep_signed(
    node: &Node,
    account: &Id,
    org: &Id,
    team: &Id,
    signed: Signed<Envelope>,
    note: DecisionNote,
) -> Result<Kept> {
    let (doc, place) = check_signed(node, account, org, team, &signed)?;
    let envelope = &signed.body;
    let is_local = node.runs_here(org, team);
    let record = DecisionRecord {
        // The replica is read as the node's own account's: for anyone else it says nothing of them.
        relayed_for: (!is_local && account == &node.account())
            .then(|| remote_relayed_for(node, &doc.body, org, team, envelope))
            .flatten(),
        ..DecisionRecord::of(envelope, note, is_local)
    };
    let _intake = node.intake();
    if let Some(kept) = already_kept(node, &record, envelope)? {
        return Ok(kept);
    }
    let parcel = Parcel::new(
        place.node,
        ParcelBody::Envelope(Box::new(signed)),
        &node.now_text(),
    );
    node.store()
        .put_parcel(&parcel)
        .map_err(|_| could_not_keep())?;
    node.store()
        .append_decision(&record)
        .map_err(|_| could_not_keep())?;
    node.ring_outbox_bell();
    Ok(Kept::Queued(None))
}

/// Where an envelope that was kept before is now; none when the node has not kept it. A different
/// envelope under the same id is a conflict, since a client that resends sends the same one.
fn already_kept(node: &Node, record: &DecisionRecord, envelope: &Envelope) -> Result<Option<Kept>> {
    let id = envelope.id.as_str();
    let conflict = || {
        Error::new(
            ErrorKind::Conflict,
            "a different decision was already kept under this envelope id",
        )
    };
    let is_other = |parcel: &Parcel| matches!(&parcel.body, ParcelBody::Envelope(kept) if kept.body != *envelope);
    let logged = node
        .store()
        .recent_decisions(usize::MAX)
        .into_iter()
        .find(|one| one.id == envelope.id);
    if logged
        .as_ref()
        .is_some_and(|one| !one.is_of(envelope, &node.account()))
    {
        return Err(conflict());
    }
    if let Some(parcel) = node
        .store()
        .pending_parcels()
        .into_iter()
        .find(|parcel| parcel.id == id)
    {
        if is_other(&parcel) {
            return Err(conflict());
        }
        if logged.is_none() {
            node.store()
                .append_decision(record)
                .map_err(|_| could_not_keep())?;
        }
        return Ok(Some(Kept::Queued(parcel.last_error)));
    }
    if let Some(parcel) = node
        .store()
        .refused_parcels()
        .into_iter()
        .find(|parcel| parcel.id == id)
    {
        if is_other(&parcel) {
            return Err(conflict());
        }
        return Ok(Some(Kept::Refused(parcel.last_error.unwrap_or_default())));
    }
    Ok(logged.map(|one| {
        if one.is_local {
            Kept::Applied
        } else {
            Kept::Delivered
        }
    }))
}

/// The background task's turn for a decision for a team on this node: applies it to the team's runtime.
/// A runtime or store that cannot act now is tried again; everything else is a refusal, which the decision
/// log shows with its reason.
pub(crate) async fn apply_here(node: &Node, signed: &Signed<Envelope>) -> Outcome {
    let envelope = &signed.body;
    match accept_envelope_for(node, &envelope.org, &envelope.team, signed.clone()).await {
        Ok((_, relayed_for)) => {
            note_relayed_for(node, &envelope.id, relayed_for);
            Outcome::Delivered
        }
        Err(error) if matches!(error.kind, ErrorKind::Unavailable | ErrorKind::Internal) => {
            Outcome::Unreachable(error.message)
        }
        Err(error) => Outcome::Refused(error.reason.map_or(error.message, |reason| reason.text)),
    }
}

/// Adds whom the decision answered for to its record, which was written before the node could know.
fn note_relayed_for(node: &Node, id: &EnvelopeId, relayed_for: Option<RelayedFor>) {
    let Some(relayed_for) = relayed_for else {
        return;
    };
    let _intake = node.intake();
    let logged = node
        .store()
        .recent_decisions(usize::MAX)
        .into_iter()
        .find(|one| &one.id == id);
    if let Some(record) = logged {
        let _ = node.store().replace_decision(&DecisionRecord {
            relayed_for: Some(relayed_for),
            ..record
        });
    }
}
