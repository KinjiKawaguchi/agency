//! A team's activity feed: read from the runtime of a team on this node. A team on another node is not
//! answered yet. Activity is never stored; each read asks the team's runtime.
use crate::client::membership;
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use agency_proto::activity::{
    valid_limit, Activity, DEFAULT_LIMIT, DEFAULT_SINCE_MINUTES, MAX_LIMIT,
};
use agency_proto::clock::{self, Duration};
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
        None => Err(Error::not_found(
            "this team's node is not asked for activity yet",
        )),
    }
}
