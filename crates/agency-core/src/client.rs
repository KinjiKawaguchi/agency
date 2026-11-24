//! The Client API's use cases: what the node's own account sees and decides through a client.
//! Only orgs the account belongs to are visible; anything else is reported as not found.
use crate::config::HostedTeam;
use crate::decisions::{decide, Decided};
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::is_root;
use crate::views::{runtime_view, viewer_name};
use agency_proto::client::{
    AccountRef, DecisionInput, DecisionOutcome, DecisionResult, Funding, Me, OrgRef,
    SnapshotSource, TeamLocation, TeamRef, TeamSnapshot,
};
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::{Id, OrgDefinition, Payload, Person, Timestamp};

/// The org, if the node's account is a member of it.
fn membership(node: &Node, org: &Id) -> Result<(SignedOrg, Person)> {
    let not_found = || Error::not_found("not found");
    let doc = node.store().load_org(org).ok_or_else(not_found)?;
    let person = doc
        .body
        .member(&node.account())
        .cloned()
        .ok_or_else(not_found)?;
    Ok((doc, person))
}

/// The hosted team, if the team runs on this node.
fn local_team(node: &Node, org: &Id, place: &TeamPlace) -> Option<HostedTeam> {
    let config = node.config();
    (place.node == config.url)
        .then(|| config.hosted(org, &place.id).cloned())
        .flatten()
}

fn team_ref(node: &Node, org: &Id, place: &TeamPlace, person: &Person) -> TeamRef {
    let (location, viewer) = match local_team(node, org, place) {
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
    if let Some(hosted) = local_team(node, org, place) {
        let view = runtime_view(node, &hosted).await?;
        return Ok(TeamSnapshot {
            view,
            viewer: viewer_name(&hosted, &person.id),
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
    Ok(TeamSnapshot {
        view,
        viewer: replica.viewer,
        source,
    })
}

/// Decides as the node's account. A refusal by the team's node is a conflict: the client should
/// re-read the team, because the request has most likely been answered or closed.
pub async fn submit_decision(
    node: &Node,
    org: &Id,
    team: &Id,
    input: DecisionInput,
) -> Result<DecisionResult> {
    membership(node, org)?;
    let payload = Payload {
        text: input.text,
        choice: input.choice,
    };
    let decided = decide(node, org, team, input.kind, input.target, payload).await?;
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
