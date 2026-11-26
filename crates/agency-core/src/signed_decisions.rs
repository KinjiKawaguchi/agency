//! Decisions signed by one of the account's devices: by a client (the Client API relays them) or by the
//! CLI device. The home node checks each one as the team's node will, then applies or sends it. It never
//! signs them itself.
use crate::decisions::{
    accept_envelope_for, send_envelope, Accepted, Decided, DecisionNote, DecisionRecord,
};
use crate::devices::DeviceSigner;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs;
use agency_proto::device::envelope_signer;
use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Id, Kind, Payload, Signed};

/// Relays a decision signed by one of this node's account's devices to its team.
pub async fn decide_signed(
    node: &Node,
    org: &Id,
    team: &Id,
    signed: Signed<Envelope>,
    note: DecisionNote,
) -> Result<Decided> {
    let envelope = &signed.body;
    if envelope.account != node.account() {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "this node relays only its own account's decisions",
        ));
    }
    if &envelope.org != org || &envelope.team != team {
        return Err(Error::bad_request(
            "the envelope's org and team differ from where it was sent",
        ));
    }
    envelope.validate().map_err(Error::bad_request)?;
    if !envelope.is_fresh(node.now()) {
        return Err(Error::bad_request(
            "the envelope expired, or its time is too far ahead",
        ));
    }
    let doc = orgs::load(node, org).map_err(|_| Error::not_found("not found"))?;
    envelope_signer(&doc.body, &signed).map_err(Error::unauthorized)?;
    let place = doc
        .body
        .team(team)
        .ok_or_else(|| Error::not_found("not found"))?
        .clone();
    let is_local = node.runs_here(org, team);
    let record = DecisionRecord::of(envelope, note, is_local);
    if !is_local {
        return send_envelope(node, place.node, signed, record).await;
    }
    let (accepted, relayed_for) = accept_envelope_for(node, org, team, signed).await?;
    if accepted == Accepted::Applied {
        let record = DecisionRecord {
            relayed_for,
            ..record
        };
        node.store().append_decision(&record)?;
    }
    Ok(Decided::Applied)
}

/// Decides with the CLI device: builds the envelope, signs it with the device key, and relays it.
pub async fn decide_with_device(
    node: &Node,
    device: DeviceSigner<'_>,
    org: &Id,
    team: &Id,
    kind: EnvelopeKind,
    target: String,
    payload: Payload,
) -> Result<Decided> {
    let envelope = Envelope {
        id: EnvelopeId::from_bytes(node.random()),
        kind,
        org: org.clone(),
        team: team.clone(),
        target,
        payload,
        account: node.account(),
        at: node.now_text(),
    };
    envelope.validate().map_err(Error::bad_request)?;
    let signed = Signed::sign(Kind::Envelope, envelope, device.id.clone(), device.key)
        .map_err(|error| Error::internal(error.to_string()))?;
    decide_signed(node, org, team, signed, DecisionNote::default()).await
}
