//! Token usage per team: read from the runtime of a team on this node, and from the team's node over a
//! signed read for a team elsewhere. Usage is never stored; each read asks the team's runtime.
use crate::client::membership;
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::ports::PeerError;
use crate::read_auth::{sign_read, verify_read, ReadAuth};
use crate::routes;
use crate::unseen;
use agency_proto::org::TeamPlace;
use agency_proto::usage::{valid_days, DEFAULT_USAGE_DAYS, MAX_USAGE_DAYS};
use agency_proto::{Id, Role, TokenUsage};

fn check_days(days: u32) -> Result<u32> {
    if valid_days(days) {
        Ok(days)
    } else {
        Err(Error::bad_request(format!(
            "days must be from 1 to {MAX_USAGE_DAYS}"
        )))
    }
}

/// Usage from the runtime of a team on this node. A runtime that does not report usage is not found.
async fn runtime_usage(node: &Node, hosted: &HostedTeam, days: u32) -> Result<TokenUsage> {
    let usage = node
        .runtime()
        .token_usage(hosted, days)
        .await
        .map_err(|error| match error.status {
            404 => Error::not_found(error.message),
            _ => Error::new(ErrorKind::Unavailable, error.message),
        })?;
    usage
        .check(&hosted.org, &hosted.team, days)
        .map_err(|message| {
            Error::new(
                ErrorKind::Unavailable,
                format!("the team runtime {message}"),
            )
        })?;
    Ok(usage)
}

/// Usage from the node a team runs on, read live. A node that cannot be reached is unavailable.
async fn remote_usage(node: &Node, org: &Id, place: &TeamPlace, days: u32) -> Result<TokenUsage> {
    let path = routes::usage(org, &place.id, days);
    let auth = sign_read(node.key(), &node.account(), "GET", &path, node.now());
    let usage = node
        .peers()
        .get_usage(&place.node, &path, &auth)
        .await
        .map_err(|error| match error {
            PeerError::Status { status: 404, .. } => Error::not_found(error.to_string()),
            _ => Error::new(
                ErrorKind::Unavailable,
                format!("the team's node did not report usage: {error}"),
            ),
        })?;
    usage.check(org, &place.id, days).map_err(|message| {
        Error::new(ErrorKind::Unavailable, format!("the team's node {message}"))
    })?;
    Ok(usage)
}

/// A team's token usage for the node's account (Client API). `days` defaults to 14.
pub async fn team_usage(node: &Node, org: &Id, team: &Id, days: Option<u32>) -> Result<TokenUsage> {
    let (doc, _) = membership(node, org)?;
    let place = doc
        .body
        .team(team)
        .ok_or_else(|| Error::not_found("not found"))?;
    let days = check_days(days.unwrap_or(DEFAULT_USAGE_DAYS))?;
    match node.hosted(org, &place.id) {
        Some(hosted) => runtime_usage(node, &hosted, days).await,
        None => remote_usage(node, org, place, days).await,
    }
}

/// The token usage of a team on this node, for a signed read by a member of its org. The signature
/// must cover the path with this `days`.
pub async fn hosted_usage(
    node: &Node,
    org: &Id,
    team: &Id,
    days: u32,
    auth: &ReadAuth,
) -> Result<TokenUsage> {
    let hosted = node.config().hosted(org, team).cloned();
    let doc = node.store().load_org(org).filter(|_| hosted.is_some());
    let reader = verify_read(
        doc.as_ref().map(|doc| &doc.body),
        auth,
        "GET",
        &routes::usage(org, team, days),
        node.now(),
    )?;
    let hosted = hosted.ok_or_else(unseen::not_found)?;
    if reader.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the team's usage",
        ));
    }
    runtime_usage(node, &hosted, check_days(days)?).await
}
