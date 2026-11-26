//! The Client API's use cases: what the node's own account sees and decides through a client.
//! Only orgs the account belongs to are visible; anything else is reported as not found.
use crate::config::NodeConfig;
use crate::decisions::{Decided, DecisionNote, DecisionRecord};
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::is_root;
use crate::outbox::Parcel;
use crate::signed_decisions::decide_signed;
use crate::views::{mine, runtime_view, viewer_name, waiting_on_viewer, Reader};
use agency_proto::client::{
    is_home_node_origin, AccountRef, DecisionEntry, DecisionLog, DecisionOutcome, DecisionResult,
    DecisionState, Funding, HomeNode, Me, OrgRef, SignedDecisionInput, SnapshotSource,
    TeamLocation, TeamRef, TeamSnapshot, MAX_NOTE,
};
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::{Id, OrgDefinition, Person, Timestamp};

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
        })
    });
    Me {
        account,
        orgs: orgs.collect(),
        funding: funding(node),
        home_nodes: None,
    }
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
        };
        let viewer = viewer_name(&hosted, &person.id);
        return Ok(TeamSnapshot {
            mine: mine(&view, &reader),
            waiting_on_viewer: waiting_on_viewer(&view, viewer.as_deref()),
            view,
            viewer,
            source: SnapshotSource::Local,
        });
    }
    let replica = node.store().load_replica(org, team).unwrap_or_default();
    let Some(view) = replica.view else {
        let why = replica.error.unwrap_or_else(|| "not read yet".to_owned());
        return Err(Error::new(
            ErrorKind::Unavailable,
            format!("no replica of {org}/{team} yet: {why}"),
        ));
    };
    let tried_at = timestamp(Some(&replica.tried_at))
        .ok_or_else(|| Error::internal("the replica's time is malformed"))?;
    let source = SnapshotSource::Replica {
        fetched_at: timestamp(replica.fetched_at.as_deref()),
        tried_at,
        error: replica.error,
    };
    let reader = Reader::Remote {
        viewer: replica.viewer.as_deref(),
    };
    Ok(TeamSnapshot {
        mine: mine(&view, &reader),
        waiting_on_viewer: waiting_on_viewer(&view, replica.viewer.as_deref()),
        view,
        viewer: replica.viewer,
        source,
    })
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

/// Relays a decision signed by one of the account's devices. The node only checks and relays it.
pub async fn submit_signed_decision(
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
    result_of(decide_signed(node, org, team, input.envelope, note).await?)
}

/// A refusal by the team's node is a conflict: the client should re-read the team, because the request
/// has most likely been answered or closed.
fn result_of(decided: Decided) -> Result<DecisionResult> {
    let (outcome, message) = match decided {
        Decided::Applied => (DecisionOutcome::Applied, None),
        Decided::Delivered => (DecisionOutcome::Delivered, None),
        Decided::Queued(why) => (DecisionOutcome::Queued, Some(why)),
        Decided::Refused(why) => {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("the team's node refused: {why}"),
            ))
        }
    };
    Ok(DecisionResult { outcome, message })
}

/// Where a logged decision is now: applied locally, waiting in the outbox, refused, or delivered.
fn state_of(
    record: &DecisionRecord,
    decided_at: &Timestamp,
    pending: &[Parcel],
    refused: &[Parcel],
) -> DecisionState {
    if record.is_local {
        return DecisionState::Applied;
    }
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
    let entries = visible.filter_map(|record| {
        let decided_at = timestamp(Some(&record.at))?;
        let state = state_of(&record, &decided_at, &pending, &refused);
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
            decided_at,
            state,
        })
    });
    DecisionLog {
        entries: entries.collect(),
    }
}

/// The other home nodes the owner configured (`homeNodes`), each with a label of 1 to 80 characters and
/// a usable origin that is not this node's own; and a line for each entry left out, for the node's log.
pub fn home_nodes(config: &NodeConfig) -> (Vec<HomeNode>, Vec<String>) {
    let mut kept: Vec<HomeNode> = vec![];
    let mut left_out = vec![];
    for entry in &config.home_nodes {
        let label = entry.label.trim();
        let url = entry.url.trim_end_matches('/');
        let reason = if !(1..=80).contains(&label.chars().count()) {
            Some("its label must be 1 to 80 characters")
        } else if !is_home_node_origin(url) {
            Some(
                "its url must be https://<host>[:<port>], or http:// on this machine, with no path",
            )
        } else if url == config.url.trim_end_matches('/') {
            Some("it is this node")
        } else if kept.iter().any(|one| one.url == url) {
            Some("it is listed twice")
        } else {
            None
        };
        match reason {
            Some(reason) => left_out.push(format!(
                "home node {:?} ({}) is left out: {reason}",
                entry.label, entry.url
            )),
            None => kept.push(HomeNode {
                label: label.to_owned(),
                url: url.to_owned(),
            }),
        }
    }
    (kept, left_out)
}
