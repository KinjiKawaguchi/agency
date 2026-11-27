//! The Client API's use cases: what the node's own account sees and decides through a client.
//! Only orgs the account belongs to are visible; anything else is reported as not found.
use crate::config::{HostedTeam, NodeConfig};
use crate::decisions::{DecisionNote, DecisionRecord};
use crate::error::{Error, ErrorKind, Result};
use crate::kept::{keep_signed, Kept};
use crate::node::Node;
use crate::orgs::is_root;
use crate::outbox::Parcel;
use crate::push::is_hiding_titles;
use crate::views::{
    addressed_party, mine, remote_addressed_party, runtime_view, viewer_name, waiting_on_viewer,
    Reader, Replica,
};
use agency_proto::client::{
    home_node_origin, AccountRef, DecisionEntry, DecisionLog, DecisionOutcome, DecisionResult,
    DecisionState, Funding, HomeNode, Me, OrgRef, SignedDecisionInput, SnapshotSource,
    StartedRecovery, TeamLocation, TeamRef, TeamSnapshot, MAX_NOTE,
};
use agency_proto::clock;
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::view::AnswersFor;
use agency_proto::{Id, OrgDefinition, Person, PublicKey, TeamView, Timestamp};
use std::collections::BTreeMap;

/// The org, if the node's account is a member of it.
pub(crate) fn membership(node: &Node, org: &Id) -> Result<(SignedOrg, Person)> {
    let not_found = || Error::not_found("not found");
    let doc = node.store().load_org(org).ok_or_else(not_found)?;
    let person = doc
        .body
        .member(&node.account())
        .cloned()
        .ok_or_else(not_found)?;
    Ok((doc, person))
}

fn team_ref(node: &Node, org: &Id, place: &TeamPlace, person: &Person) -> TeamRef {
    let (location, viewer) = match node.hosted(org, &place.id) {
        Some(hosted) => (TeamLocation::Local, viewer_name(&hosted, &person.id)),
        None => (
            TeamLocation::Remote,
            node.store()
                .load_replica(org, &place.id)
                .and_then(|replica| replica.viewer),
        ),
    };
    TeamRef {
        id: place.id.clone(),
        label: place.label.clone(),
        location,
        viewer,
    }
}

fn funding(node: &Node) -> Vec<Funding> {
    let sources = node.config().funding;
    let funded = sources.into_iter().map(|source| {
        let (windows, read_at) = node.funding().usage(&source);
        Funding {
            id: source.id,
            label: source.label,
            kind: source.kind,
            windows,
            read_at,
        }
    });
    funded.collect()
}

pub fn me(node: &Node) -> Me {
    let config = node.config();
    let fingerprint = node.key().public().fingerprint();
    let account = AccountRef {
        id: config.account.id.clone(),
        label: config.account.label.clone(),
        fingerprint,
    };
    let orgs = node.store().orgs().into_iter().filter_map(|doc| {
        let person = doc.body.member(&config.account.id)?.clone();
        let teams = doc
            .body
            .teams
            .iter()
            .map(|place| team_ref(node, &doc.body.org, place, &person))
            .collect();
        Some(OrgRef {
            id: doc.body.org.clone(),
            label: doc.body.label.clone(),
            is_root: is_root(node, &doc),
            teams,
            recovery: started_recovery(node, &doc),
        })
    });
    Me {
        account,
        orgs: orgs.collect(),
        funding: funding(node),
        hides_titles: is_hiding_titles(node),
        node_id: agency_proto::client::node_id(&config.url),
        home_nodes: None,
        root_changes: None,
    }
}

/// The recovery of `doc` that waits, if one does.
fn started_recovery(node: &Node, doc: &SignedOrg) -> Option<StartedRecovery> {
    let pending = doc.body.root.pending.as_ref()?;
    Some(StartedRecovery {
        takes_effect_at: crate::orgs::recovery_takes_effect(node, doc).map(clock::format),
        new_root_key: pending.public_key.as_ref().map(PublicKey::fingerprint),
        replaces_recovery_key: pending.recovery.is_some(),
    })
}

/// The current definition of an org the account belongs to.
pub fn org_definition(node: &Node, org: &Id) -> Result<OrgDefinition> {
    membership(node, org).map(|(doc, _)| doc.body)
}

fn timestamp(raw: Option<&str>) -> Option<Timestamp> {
    raw.and_then(|raw| Timestamp::parse(raw).ok())
}

/// A team's view: read from the runtime for a team on this node, or the last replica otherwise.
pub async fn team_snapshot(node: &Node, org: &Id, team: &Id) -> Result<TeamSnapshot> {
    let (doc, person) = membership(node, org)?;
    let place = doc
        .body
        .team(team)
        .ok_or_else(|| Error::not_found("not found"))?;
    if let Some(hosted) = node.hosted(org, &place.id) {
        let view = runtime_view(node, &hosted).await?;
        let reader = Reader::Hosted {
            hosted: &hosted,
            person: &person.id,
            definition: &doc.body,
        };
        let viewer = viewer_name(&hosted, &person.id);
        let mine = mine(&view, &reader);
        return Ok(TeamSnapshot {
            relayed_for: relayed_for(&doc.body, &hosted, &view, &mine),
            mine,
            waiting_on_viewer: waiting_on_viewer(&view, viewer.as_deref()),
            view,
            viewer,
            source: SnapshotSource::Local,
        });
    }
    let replica = node.store().load_replica(org, team).unwrap_or_default();
    let Replica {
        fetched_at,
        tried_at,
        error,
        view,
        viewer,
        answers_for,
    } = replica;
    let Some(view) = view else {
        let why = error.unwrap_or_else(|| "not read yet".to_owned());
        return Err(Error::new(
            ErrorKind::Unavailable,
            format!("no replica of {org}/{team} yet: {why}"),
        ));
    };
    let tried_at = timestamp(Some(&tried_at))
        .ok_or_else(|| Error::internal("the replica's time is malformed"))?;
    let reader = Reader::Remote {
        viewer: viewer.as_deref(),
        answers_for: &answers_for,
    };
    let mine = mine(&view, &reader);
    let source = SnapshotSource::Replica {
        fetched_at: timestamp(fetched_at.as_deref()),
        tried_at,
        error,
    };
    Ok(TeamSnapshot {
        relayed_for: remote_relayed_for(&doc.body, &answers_for, &view, &mine),
        waiting_on_viewer: waiting_on_viewer(&view, viewer.as_deref()),
        mine,
        view,
        viewer,
        source,
    })
}

/// For each of the reader's requests that is addressed to a seat or an external party: whom it is for.
fn relayed_for(
    definition: &OrgDefinition,
    hosted: &HostedTeam,
    view: &TeamView,
    mine: &[String],
) -> BTreeMap<String, String> {
    let requests = view.requests.iter().filter(|one| mine.contains(&one.id));
    requests
        .filter_map(|request| {
            let (_, label) = addressed_party(definition, hosted, request.to.as_deref())?;
            Some((request.id.clone(), label.to_owned()))
        })
        .collect()
}

/// `relayed_for` for a team on another node: the seats and external parties its node says the reader
/// answers for, with their labels from this node's definition of the org.
fn remote_relayed_for(
    definition: &OrgDefinition,
    answers_for: &[AnswersFor],
    view: &TeamView,
    mine: &[String],
) -> BTreeMap<String, String> {
    let requests = view.requests.iter().filter(|one| mine.contains(&one.id));
    requests
        .filter_map(|request| {
            let (_, label) =
                remote_addressed_party(definition, answers_for, request.to.as_deref())?;
            Some((request.id.clone(), label.to_owned()))
        })
        .collect()
}

/// A note's text, trimmed; empty is none. At most `MAX_NOTE` characters.
fn note_text(raw: Option<String>, field: &str) -> Result<Option<String>> {
    let text = raw
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    if text
        .as_ref()
        .is_some_and(|text| text.chars().count() > MAX_NOTE)
    {
        return Err(Error::bad_request(format!(
            "{field} is at most {MAX_NOTE} characters"
        )));
    }
    Ok(text)
}

/// Keeps a decision signed by one of the account's devices and answers at once; the node applies or sends it
/// in the background (see `kept`). The same envelope sent again gets the answer for where it is now.
pub fn submit_signed_decision(
    node: &Node,
    org: &Id,
    team: &Id,
    input: SignedDecisionInput,
) -> Result<DecisionResult> {
    membership(node, org)?;
    let note = DecisionNote {
        title: note_text(input.title, "title")?,
        choice_label: note_text(input.choice_label, "choiceLabel")?,
    };
    if note.choice_label.is_some() && input.envelope.body.payload.choice.is_none() {
        return Err(Error::bad_request("choiceLabel needs a choice"));
    }
    result_of(keep_signed(node, org, team, input.envelope, note)?)
}

/// A refusal by the team's node or runtime is a conflict: the client should re-read the team, because the
/// request has most likely been answered or closed.
fn result_of(kept: Kept) -> Result<DecisionResult> {
    let (outcome, message) = match kept {
        Kept::Applied => (DecisionOutcome::Applied, None),
        Kept::Delivered => (DecisionOutcome::Delivered, None),
        Kept::Queued(why) => (DecisionOutcome::Queued, why),
        Kept::Refused(why) => {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("the team's node refused: {why}"),
            ))
        }
    };
    Ok(DecisionResult { outcome, message })
}

/// Where a logged decision is now: waiting in the outbox, refused, or (once nothing waits for it)
/// applied to a team on this node or delivered to the team's node.
fn state_of(
    record: &DecisionRecord,
    decided_at: &Timestamp,
    pending: &[Parcel],
    refused: &[Parcel],
) -> DecisionState {
    let id = record.id.as_str();
    if let Some(parcel) = pending.iter().find(|parcel| parcel.id == id) {
        let next_at = timestamp(Some(&parcel.next_at)).unwrap_or_else(|| decided_at.clone());
        return DecisionState::Queued {
            attempts: parcel.attempts,
            next_at,
            last_error: parcel.last_error.clone(),
        };
    }
    match refused.iter().find(|parcel| parcel.id == id) {
        Some(parcel) => DecisionState::Refused {
            message: parcel.last_error.clone().unwrap_or_default(),
        },
        None if record.is_local => DecisionState::Applied,
        None => DecisionState::Delivered,
    }
}

/// Default and largest number of entries `decision_log` returns.
pub const DEFAULT_LOG_LIMIT: usize = 50;
pub const MAX_LOG_LIMIT: usize = 200;

/// The account's recent decisions in orgs it still belongs to, newest first, with where each one went.
pub fn decision_log(node: &Node, limit: Option<usize>) -> DecisionLog {
    let limit = limit.unwrap_or(DEFAULT_LOG_LIMIT).clamp(1, MAX_LOG_LIMIT);
    let (pending, refused) = (
        node.store().pending_parcels(),
        node.store().refused_parcels(),
    );
    let visible = node
        .store()
        .recent_decisions(limit)
        .into_iter()
        .filter(|record| membership(node, &record.org).is_ok());
    DecisionLog {
        entries: visible
            .filter_map(|record| entry_of(record, &pending, &refused))
            .collect(),
    }
}

/// One logged decision of the account, with where it is now. Not found for an id the log does not hold
/// (it keeps the last 500), and for a decision in an org the account no longer belongs to.
pub fn decision_entry(node: &Node, id: &str) -> Result<DecisionEntry> {
    let (pending, refused) = (
        node.store().pending_parcels(),
        node.store().refused_parcels(),
    );
    node.store()
        .recent_decisions(usize::MAX)
        .into_iter()
        .find(|record| record.id.as_str() == id && membership(node, &record.org).is_ok())
        .and_then(|record| entry_of(record, &pending, &refused))
        .ok_or_else(|| Error::not_found("not found"))
}

fn entry_of(
    record: DecisionRecord,
    pending: &[Parcel],
    refused: &[Parcel],
) -> Option<DecisionEntry> {
    let decided_at = timestamp(Some(&record.at))?;
    let state = state_of(&record, &decided_at, pending, refused);
    Some(DecisionEntry {
        id: record.id.as_str().to_owned(),
        org: record.org,
        team: record.team,
        kind: record.kind,
        target: record.target,
        title: record.title,
        text: record.payload.text,
        choice: record.payload.choice,
        choice_label: record.choice_label,
        relayed_for: record.relayed_for,
        decided_at,
        state,
    })
}

/// The other home nodes the owner configured (`homeNodes`), each with a label of 1 to 80 characters and
/// a usable origin that is not this node's own; and a line for each entry left out, for the node's log.
pub fn home_nodes(config: &NodeConfig) -> (Vec<HomeNode>, Vec<String>) {
    let mut kept: Vec<HomeNode> = vec![];
    let mut left_out = vec![];
    let own = home_node_origin(&config.url);
    for entry in &config.home_nodes {
        let label = entry.label.trim();
        let url = home_node_origin(&entry.url);
        let reason = match &url {
            _ if !(1..=80).contains(&label.chars().count()) => {
                Some("its label must be 1 to 80 characters")
            }
            None => Some(
                "its url must be https://<host>[:<port>], or http:// on this machine, with no path",
            ),
            Some(url) if Some(url) == own.as_ref() => Some("it is this node"),
            Some(url) if kept.iter().any(|one| &one.url == url) => Some("it is listed twice"),
            Some(_) => None,
        };
        match (reason, url) {
            (None, Some(url)) => kept.push(HomeNode {
                label: label.to_owned(),
                url,
            }),
            (reason, _) => left_out.push(format!(
                "home node {:?} ({}) is left out: {}",
                entry.label,
                entry.url,
                reason.unwrap_or("its url is not usable")
            )),
        }
    }
    (kept, left_out)
}
