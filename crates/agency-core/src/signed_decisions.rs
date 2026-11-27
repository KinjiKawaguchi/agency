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
use crate::views::{is_addressed_to, remote_addressed_party, Reader};
use agency_proto::client::RelayedFor;
use agency_proto::device::envelope_signer;
use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Id, Kind, OrgDefinition, Payload, Signed};

/// Relays a decision signed by one of this node's account's devices to its team.
pub async fn decide_signed(
    node: &Node,
    account: &Id,
    org: &Id,
    team: &Id,
    signed: Signed<Envelope>,
    note: DecisionNote,
) -> Result<Decided> {
    let envelope = &signed.body;
    // The account the caller acts as: the node's own, or the hosted member a guest is.
    if &envelope.account != account {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "this node relays only the decisions of the account the caller acts as",
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
    let record = DecisionRecord::of(&signed, note, is_local);
    if !is_local {
        let record = DecisionRecord {
            // The replica is read as the node's own account's: for anyone else it says nothing of them.
            relayed_for: (account == &node.account())
                .then(|| remote_relayed_for(node, &doc.body, org, team, envelope))
                .flatten(),
            ..record
        };
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

/// Whom a decision for a team on another node answers for, as this node's last replica of the team and its
/// definition of the org have it: the seat or external party the decided request is addressed to, when the
/// team's node said the signer answers for it. The team's node decides whether to accept the decision; this
/// is only what the decision log says it was made for.
fn remote_relayed_for(
    node: &Node,
    definition: &OrgDefinition,
    org: &Id,
    team: &Id,
    envelope: &Envelope,
) -> Option<RelayedFor> {
    let replica = node.store().load_replica(org, team)?;
    let reader = Reader::of_replica(&replica);
    let view = replica.view.as_ref()?;
    let request = view.requests.iter().find(|request| {
        request.answer_with.kind == envelope.kind
            && request.answer_with.target == envelope.target
            && is_addressed_to(&reader, request)
    })?;
    let (id, label) = remote_addressed_party(
        definition,
        &replica.answers_for,
        &envelope.account,
        request.to.as_deref(),
    )?;
    Some(RelayedFor {
        id: id.clone(),
        label: label.to_owned(),
    })
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
    let account = node.account();
    decide_signed(node, &account, org, team, signed, DecisionNote::default()).await
}
