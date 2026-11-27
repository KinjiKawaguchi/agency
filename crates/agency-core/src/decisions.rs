//! Decisions (kind `envelope`). A person's decision is signed with one of their devices; no node signs one.
//! The team's node verifies it before applying it to the team runtime.
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::outbox::{deliver, enqueue, Outcome, Parcel, ParcelBody};
use crate::unseen;
use crate::views::{self, is_addressed_to, Reader};
use agency_proto::client::RelayedFor;
use agency_proto::device::envelope_key;
use agency_proto::runtime::RuntimeError;
use agency_proto::Kind;
use agency_proto::{
    Envelope, EnvelopeId, EnvelopeKind, Id, OrgDefinition, Payload, RuntimeDecision, Signed,
};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relayed_for: Option<RelayedFor>,
    pub payload: Payload,
    pub at: String,
    /// Applied to a team on this node; otherwise sent as an envelope, whose delivery the outbox tracks.
    pub is_local: bool,
    /// The member who decided: the node's own account, or a member it hosts (a guest). Absent in records
    /// made before guests, which were all the node's own account's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Id>,
}

impl DecisionRecord {
    /// Whether `account` made this decision, a record without `by` being the node's own account's.
    pub(crate) fn is_by(&self, account: &Id, node_account: &Id) -> bool {
        self.by.as_ref().unwrap_or(node_account) == account
    }
}

/// What people read about a decision once its request is gone: the request's title and the label of the
/// chosen option. Kept in the decision log only; never signed or sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionNote {
    pub title: Option<String>,
    pub choice_label: Option<String>,
}

impl DecisionRecord {
    /// Whether the record is of this very envelope, so that one sent again is the one that was kept.
    pub(crate) fn is_of(&self, envelope: &Envelope, node_account: &Id) -> bool {
        self.is_by(&envelope.account, node_account)
            && (
                &self.id,
                &self.org,
                &self.team,
                self.kind,
                &self.target,
                &self.payload,
                &self.at,
            ) == (
                &envelope.id,
                &envelope.org,
                &envelope.team,
                envelope.kind,
                &envelope.target,
                &envelope.payload,
                &envelope.at,
            )
    }

    pub(crate) fn of(envelope: &Envelope, note: DecisionNote, is_local: bool) -> Self {
        Self {
            id: envelope.id.clone(),
            org: envelope.org.clone(),
            team: envelope.team.clone(),
            kind: envelope.kind,
            target: envelope.target.clone(),
            title: note.title,
            choice_label: note.choice_label,
            relayed_for: None,
            payload: envelope.payload.clone(),
            at: envelope.at.clone(),
            is_local,
            by: Some(envelope.account.clone()),
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

/// Only the request's addressee decides it, by the rule that also gives each reader their requests
/// (`views::is_addressed_to`, the same rule as `mine`): the member the request's `to` names in the
/// hosted team's `people`, or the team's owner when `to` is absent or unmapped. The request is the one
/// whose `answerWith` is the envelope's kind and target, in the team's view now. A request that is not
/// waiting and a request addressed to someone else get the same refusal, so an answerer, who reads only
/// their own requests, cannot learn of others' from it.
///
/// It gives the seat or external party the request is addressed to, with its label as `definition` has it,
/// when the decider answers for it.
async fn check_addressee(
    node: &Node,
    hosted: &HostedTeam,
    definition: &OrgDefinition,
    envelope: &Envelope,
    person: &Id,
) -> Result<Option<RelayedFor>> {
    let view = views::runtime_view(node, hosted).await?;
    let reader = Reader::Hosted {
        hosted,
        person,
        definition,
    };
    let found = view.requests.iter().find(|request| {
        request.answer_with.kind == envelope.kind
            && request.answer_with.target == envelope.target
            && is_addressed_to(&reader, request)
    });
    match found {
        Some(request) => Ok(
            views::addressed_party(definition, hosted, request.to.as_deref()).map(|(id, label)| {
                RelayedFor {
                    id: id.clone(),
                    label: label.to_owned(),
                }
            }),
        ),
        None => Err(Error::new(
            ErrorKind::Forbidden,
            format!(
                "no request {} waits for a decision from {person}",
                envelope.target
            ),
        )),
    }
}

/// Verifies an envelope sent to a team on this node and applies it. Checked in order: shape, where it
/// was sent, membership and signature, expiry, replay, and that the decider is the request's addressee.
/// An unknown org or team, a non-member, an unknown device, and a wrong signature are all refused like
/// an unknown team (see `unseen`).
pub async fn accept_envelope(
    node: &Node,
    org: &Id,
    team: &Id,
    signed: Signed<Envelope>,
) -> Result<Accepted> {
    Ok(accept_envelope_for(node, org, team, signed).await?.0)
}

/// `accept_envelope`, and whom the decision answered for, as this node's definition of the org says.
pub(crate) async fn accept_envelope_for(
    node: &Node,
    org: &Id,
    team: &Id,
    signed: Signed<Envelope>,
) -> Result<(Accepted, Option<RelayedFor>)> {
    let envelope = &signed.body;
    envelope.validate().map_err(Error::bad_request)?;
    if &envelope.org != org || &envelope.team != team {
        return Err(Error::bad_request(
            "the envelope's org and team differ from where it was sent",
        ));
    }
    let hosted = node.hosted(org, team);
    let doc = node.store().load_org(org);
    let definition = doc.as_ref().map(|doc| &doc.body);
    let found = definition.and_then(|definition| envelope_key(definition, &signed));
    let is_valid = unseen::check(&signed, Kind::Envelope, found.map(|(_, key)| key));
    let (account, signer) = (&envelope.account, &signed.signer);
    let person = match (definition, found) {
        (None, _) => {
            return Err(unseen::refused(format!(
                "an envelope from {account} for {org}, an org this node does not have"
            )))
        }
        (Some(definition), None) if definition.member(account).is_none() => {
            return Err(unseen::refused(format!(
                "an envelope from {account}, who is not a member of {org}"
            )))
        }
        (Some(_), None) => {
            return Err(unseen::refused(format!(
                "an envelope from {account} signed as {signer}, which is not one of their devices"
            )))
        }
        (Some(_), Some(_)) if !is_valid => {
            return Err(unseen::refused(format!(
                "an envelope from {account} whose signature does not match device {signer}"
            )))
        }
        (Some(_), Some((person, _))) => person,
    };
    let Some(hosted) = hosted else {
        return Err(unseen::refused_member(format!(
            "an envelope from {account} for {org}/{team}, which does not run on this node"
        )));
    };
    if !envelope.is_fresh(node.now()) {
        return Err(Error::bad_request(
            "the envelope expired, or its time is too far ahead",
        ));
    }
    let _guard = node.write_lock().lock().await;
    if node.store().is_applied(&envelope.id) {
        return Ok((Accepted::Duplicate, None));
    }
    let definition = definition.ok_or_else(|| Error::internal("the org's definition is gone"))?;
    let on_behalf_of = check_addressee(node, &hosted, definition, envelope, &person.id).await?;
    let decision = RuntimeDecision {
        kind: envelope.kind,
        target: envelope.target.clone(),
        by: person.id.clone(),
        by_participant: views::viewer_name(&hosted, &person.id),
        on_behalf_of: on_behalf_of.as_ref().map(|party| party.id.to_string()),
        payload: envelope.payload.clone(),
    };
    node.runtime()
        .decide(&hosted, decision)
        .await
        .map_err(runtime_refusal)?;
    node.store()
        .record_applied(&envelope.id, &node.now_text())?;
    Ok((Accepted::Applied, on_behalf_of))
}
