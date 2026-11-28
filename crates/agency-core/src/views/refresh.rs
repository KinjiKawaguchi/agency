//! Reading the teams on other nodes: each replica, the org's definition from another of its nodes, and the wait
//! for a node that does not answer.
use super::served::check_view;
use super::Replica;
use crate::error::Result;
use crate::node::Node;
use crate::orgs;
use crate::peer_about;
use crate::ports::{DefinitionAccess, PeerError, ViewRead};
use crate::read_auth::sign_for_peer;
use crate::routes;
use agency_proto::about::Capability;
use agency_proto::clock;
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::view::FederatedView;
use agency_proto::Id;

/// Why a team's view could not be read from its node, and whether that node answered 404 (the peer may not
/// be the release this node read it as).
struct FetchFailure {
    message: String,
    is_not_found: bool,
}

/// What a read of a team's view came to: the answer, checked, and the tag the node gave it; or that the answer this
/// node holds is still the one.
enum Fetched {
    Changed(Box<FederatedView>, Option<String>),
    Unchanged,
}

async fn fetch_view(
    node: &Node,
    org: &Id,
    place: &TeamPlace,
    have: Option<&str>,
) -> Result<Fetched, FetchFailure> {
    let path = routes::view(org, &place.id);
    let auth = sign_for_peer(node, org, &place.node, &path);
    let read = node
        .peers()
        .get_view(&place.node, &path, &auth, have)
        .await
        .map_err(|error| FetchFailure {
            is_not_found: matches!(error, PeerError::Status { status: 404, .. }),
            message: error.to_string(),
        })?;
    let other = |message: String| FetchFailure {
        message,
        is_not_found: false,
    };
    let (fetched, tag) = match read {
        ViewRead::Unchanged if have.is_some() => return Ok(Fetched::Unchanged),
        ViewRead::Unchanged => {
            return Err(other(
                "answered that the view is unchanged to a read that held none".into(),
            ))
        }
        ViewRead::Answer { answer, tag } => (answer, tag),
    };
    fetched
        .check_answers_for()
        .map_err(|over| other(format!("sent an answer beyond the limits: {over}")))?;
    let view = check_view(fetched.view, org, &place.id).map_err(other)?;
    Ok(Fetched::Changed(
        Box::new(FederatedView {
            view,
            viewer: fetched.viewer,
            answers_for: fetched.answers_for,
        }),
        tag,
    ))
}

async fn refresh_replica(node: &Node, org: &Id, place: &TeamPlace) -> Result<()> {
    let previous = node
        .store()
        .load_replica(org, &place.id)
        .unwrap_or_default();
    let now = node.now_text();
    // The tag is sent only for a view this node holds, to a node that lists `viewUnchanged`.
    let have = previous.tag.as_deref().filter(|_| {
        previous.view.is_some()
            && peer_about::lists(node, org, &place.node, Capability::ViewUnchanged)
    });
    let next = match fetch_view(node, org, place, have).await {
        Ok(Fetched::Changed(fetched, tag)) => Replica {
            fetched_at: Some(now.clone()),
            tried_at: now,
            error: None,
            view: Some(fetched.view),
            viewer: fetched.viewer,
            answers_for: fetched.answers_for,
            failures: 0,
            failing_since: None,
            tag,
        },
        // The view this node holds is still the answer: nothing to keep but that it was read just now, and that the
        // team's node answered (which ends a wait).
        Ok(Fetched::Unchanged) => Replica {
            fetched_at: Some(now.clone()),
            tried_at: now,
            error: None,
            failures: 0,
            failing_since: None,
            ..previous
        },
        Err(failure) => {
            if failure.is_not_found {
                peer_about::note_not_found(node, org, &place.node)?;
            }
            Replica {
                failures: previous.failures.saturating_add(1),
                failing_since: previous.failing_since.clone().or_else(|| Some(now.clone())),
                tried_at: now,
                error: Some(failure.message),
                ..previous
            }
        }
    };
    node.store().save_replica(org, &place.id, &next)
}

/// Takes a newer org definition from one of the org's other nodes, if it has one: a node whose team was last read
/// without an error, so that a node that does not answer is not the one asked every time.
async fn refresh_definition(node: &Node, doc: &SignedOrg) {
    let org = &doc.body.org;
    let elsewhere: Vec<TeamPlace> = doc
        .body
        .teams()
        .filter(|place| !node.runs_here(org, &place.id))
        .collect();
    let answered = |place: &&TeamPlace| {
        node.store()
            .load_replica(org, &place.id)
            .is_some_and(|replica| replica.error.is_none() && replica.fetched_at.is_some())
    };
    let Some(place) = elsewhere
        .iter()
        .find(answered)
        .or_else(|| elsewhere.first())
    else {
        return;
    };
    let path = routes::definition(&doc.body.org);
    let access = DefinitionAccess::Signed(sign_for_peer(node, &doc.body.org, &place.node, &path));
    if let Ok(newer) = node
        .peers()
        .get_definition(&place.node, &path, &access)
        .await
    {
        if newer.body.version > doc.body.version + 1 {
            let _ = orgs::catch_up(node, &place.node, org, newer.body.version).await;
        }
        let _ = orgs::receive(node, newer).await;
    }
}

/// What `agency status` says of the teams whose node is being waited for: one line each, with since when its reads
/// fail, how many failed, and when it is read next.
pub fn waiting_lines(node: &Node) -> Vec<String> {
    let account = node.account();
    let mut lines = vec![];
    for doc in node.store().orgs() {
        if doc.body.member(&account).is_none() {
            continue;
        }
        let org = &doc.body.org;
        for place in doc
            .body
            .teams()
            .filter(|place| !node.runs_here(org, &place.id))
        {
            let Some(replica) = node.store().load_replica(org, &place.id) else {
                continue;
            };
            let Some(next) = replica.next_read_at() else {
                continue;
            };
            lines.push(format!(
                "Team {org}/{} on {}: not answering since {} ({} tries); next try at {}",
                place.id,
                place.node,
                replica.failing_since.as_deref().unwrap_or("?"),
                replica.failures,
                clock::format(next)
            ));
        }
    }
    lines
}

/// How many teams on other nodes are read at once. A team node that does not answer holds one of these for its
/// timeout, not the whole pass.
pub const READS_AT_ONCE: usize = 8;

/// Re-reads the replicas of every team on other nodes in the orgs this node's account belongs to, at most
/// `READS_AT_ONCE` at a time, and, at the same time, org by org, what the org's other nodes say about themselves
/// and the org's definition. The first failure to keep what was read is returned once everything was tried.
pub async fn refresh(node: &Node) -> Result<()> {
    refresh_teams(node, Due::Only).await
}

/// `refresh`, reading every team now, those whose node is being waited for included (`agency sync`).
pub async fn refresh_now(node: &Node) -> Result<()> {
    refresh_teams(node, Due::All).await
}

/// Which teams on other nodes a pass reads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Due {
    /// Those whose node answered last time, or whose wait after failing is over.
    Only,
    All,
}

/// Whether the team's replica should be read now: it is read when its node answered last time, when its wait after
/// failing is over, and when the team's node has answered this node in another way since the read that failed (an
/// outbox parcel it took or refused, its `about`: `Node::note_answer`).
fn is_due(node: &Node, org: &Id, place: &TeamPlace, due: Due) -> bool {
    if due == Due::All {
        return true;
    }
    let Some(replica) = node.store().load_replica(org, &place.id) else {
        return true;
    };
    let Some(next) = replica.next_read_at() else {
        return true;
    };
    let tried_at = clock::parse(&replica.tried_at);
    let has_answered_since = node
        .answered_at(&place.node)
        .zip(tried_at)
        .is_some_and(|(answered, tried)| answered > tried);
    next <= node.now() || has_answered_since
}

async fn refresh_teams(node: &Node, due: Due) -> Result<()> {
    use futures_util::stream::{self, StreamExt};
    let account = node.account();
    let orgs: Vec<SignedOrg> = node
        .store()
        .orgs()
        .into_iter()
        .filter(|doc| doc.body.member(&account).is_some())
        .collect();
    let elsewhere: Vec<(Id, TeamPlace)> = orgs
        .iter()
        .flat_map(|doc| {
            let org = &doc.body.org;
            doc.body
                .teams()
                .filter(|place| !node.runs_here(org, &place.id))
                .filter(|place| is_due(node, org, place, due))
                .map(|place| (org.clone(), place.clone()))
        })
        .collect();
    let replicas = stream::iter(elsewhere)
        .map(|(org, place)| async move { refresh_replica(node, &org, &place).await })
        .buffer_unordered(READS_AT_ONCE)
        .collect::<Vec<Result<()>>>();
    let peers = stream::iter(orgs)
        .map(|doc| async move {
            let (about, ()) = futures_util::future::join(
                peer_about::refresh(node, &doc.body),
                refresh_definition(node, &doc),
            )
            .await;
            about
        })
        .buffer_unordered(READS_AT_ONCE)
        .collect::<Vec<Result<()>>>();
    let (replicas, peers) = futures_util::future::join(replicas, peers).await;
    replicas.into_iter().chain(peers).collect()
}
