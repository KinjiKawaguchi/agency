//! A team's activity feed: read from the runtime of a team on this node, and from the team's node over a
//! signed read for a team elsewhere. Activity is never stored; each read asks the team's runtime.
use crate::client::membership;
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::peer_features::{lacking_reason, support_of_peer, Support};
use crate::ports::PeerError;
use crate::read_auth::{sign_for_peer, verify_read_once, ReadAuth};
use crate::routes;
use crate::unseen;
use agency_proto::about::Capability;
use agency_proto::activity::{
    valid_limit, Activity, DEFAULT_LIMIT, DEFAULT_SINCE_MINUTES, MAX_LIMIT,
};
use agency_proto::clock::{self, Duration};
use agency_proto::org::TeamPlace;
use agency_proto::{Id, Role, Timestamp};

fn check_limit(limit: u32) -> Result<u32> {
    if valid_limit(limit) {
        Ok(limit)
    } else {
        Err(Error::bad_request(format!(
            "limit must be from 1 to {MAX_LIMIT}"
        )))
    }
}

/// The events of a team on this node after `since`, from its runtime. A runtime that does not report
/// activity is not found; one whose answer is malformed is unavailable.
async fn runtime_activity(
    node: &Node,
    hosted: &HostedTeam,
    since: &Timestamp,
    limit: u32,
) -> Result<Activity> {
    let activity = node
        .runtime()
        .activity(hosted, since, limit)
        .await
        .map_err(|error| match error.status {
            404 => Error::not_found(error.message),
            _ => Error::new(ErrorKind::Unavailable, error.message),
        })?;
    activity
        .check(&hosted.org, &hosted.team, since, limit)
        .map_err(|message| {
            Error::new(
                ErrorKind::Unavailable,
                format!("the team runtime {message}"),
            )
        })?;
    Ok(activity)
}

/// A team's activity for the node's account (Client API): at most `limit` events (default 200) after
/// `since` (default: an hour ago), oldest first. An answerer reads none of it, like the team's usage.
pub async fn team_activity(
    node: &Node,
    org: &Id,
    team: &Id,
    since: Option<Timestamp>,
    limit: Option<u32>,
) -> Result<Activity> {
    let (doc, person) = membership(node, org)?;
    let place = doc
        .body
        .team(team)
        .ok_or_else(|| Error::not_found("not found"))?;
    if person.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the team's activity",
        ));
    }
    let limit = check_limit(limit.unwrap_or(DEFAULT_LIMIT))?;
    let since = match since {
        Some(since) => since,
        None => {
            let hour_ago = node.now() - Duration::minutes(DEFAULT_SINCE_MINUTES);
            Timestamp::parse(&clock::format(hour_ago))
                .map_err(|error| Error::internal(error.to_string()))?
        }
    };
    match node.hosted(org, &place.id) {
        Some(hosted) => runtime_activity(node, &hosted, &since, limit).await,
        None => remote_activity(node, org, place, &since, limit).await,
    }
}

/// The events from the node a team runs on, read live. A node that cannot be reached, or whose answer is
/// malformed, is unavailable, and so is a node that has been read and does not list the `activity`
/// capability (the route is not called, and the reason names the release it runs); a node that lists it, or
/// has not been read, and does not report activity or does not serve the route is not found; a node that
/// refuses the reader (403) is passed through.
async fn remote_activity(
    node: &Node,
    org: &Id,
    place: &TeamPlace,
    since: &Timestamp,
    limit: u32,
) -> Result<Activity> {
    if let Support::Lacking { release } =
        support_of_peer(node, org, &place.node, Capability::Activity)
    {
        return Err(Error::new(
            ErrorKind::Unavailable,
            lacking_reason("the team's activity feed", &release),
        ));
    }
    let path = routes::activity(org, &place.id, since, limit);
    let auth = sign_for_peer(node, org, &place.node, &path);
    let activity = node
        .peers()
        .get_activity(&place.node, &path, &auth)
        .await
        .map_err(|error| match error {
            PeerError::Status { status: 404, .. } => Error::not_found(error.to_string()),
            PeerError::Status { status: 403, .. } => Error::new(
                ErrorKind::Forbidden,
                "the team's node does not let this account read the team's activity",
            ),
            _ => Error::new(
                ErrorKind::Unavailable,
                format!("the team's node did not report activity: {error}"),
            ),
        })?;
    activity
        .check(org, &place.id, since, limit)
        .map_err(|message| {
            Error::new(ErrorKind::Unavailable, format!("the team's node {message}"))
        })?;
    Ok(activity)
}

/// What a signed read of a team's activity asked: `since` and `limit` as the query wrote them (each absent
/// or as text), and the path with its query as it was received.
pub struct Asked<'a> {
    pub since: Option<&'a str>,
    pub limit: Option<&'a str>,
    pub received: &'a str,
}

/// The events of a team on this node, for a signed read by a member of its org. The signature must cover
/// the path with this `since` and `limit`; a query that is not well formed is checked against the path as
/// received, so that no caller learns about it before it is authenticated.
pub async fn hosted_activity(
    node: &Node,
    org: &Id,
    team: &Id,
    asked: &Asked<'_>,
    auth: &ReadAuth,
) -> Result<Activity> {
    let parsed = asked
        .since
        .ok_or("since is needed")
        .and_then(|raw| Timestamp::parse(raw).map_err(|_| "since must be an RFC 3339 UTC time"))
        .and_then(|since| {
            let limit = asked.limit.ok_or("limit is needed")?;
            let limit = limit.parse::<u32>().map_err(|_| "limit must be a number")?;
            Ok((since, limit))
        });
    let signed_path = match &parsed {
        Ok((since, limit)) => routes::activity(org, team, since, *limit),
        Err(_) => asked.received.to_owned(),
    };
    let hosted = node.hosted(org, team);
    let doc = node.store().load_org(org);
    let reader = verify_read_once(
        node,
        doc.as_ref().map(|doc| &doc.body),
        hosted.is_some(),
        auth,
        "GET",
        &signed_path,
    )?;
    let hosted =
        hosted.ok_or_else(|| unseen::refused("a read of a team that does not run here"))?;
    if reader.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the team's activity",
        ));
    }
    let (since, limit) = parsed.map_err(Error::bad_request)?;
    runtime_activity(node, &hosted, &since, check_limit(limit)?).await
}
