//! A team's effort (docs/protocol.md, "Evaluation", `Effort`): where its tokens went, by work type.
//! Worked out on the node the team runs on from its repositories on GitHub and its token usage, and
//! passed on to other nodes over a signed read. Nothing is stored here.
use crate::client::membership;
use crate::config::HostedTeam;
use crate::delivery::{effort, EffortInput, Period};
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::ports::{DeliveryError, PeerError};
use crate::read_auth::{sign_read, verify_read_once};
use crate::routes;
use crate::unseen;
use crate::usage::check_days;
use agency_proto::effort::Effort;
use agency_proto::evaluation::TeamUnavailable;
use agency_proto::org::TeamPlace;
use agency_proto::usage::{TokenCounts, DEFAULT_USAGE_DAYS};
use agency_proto::{Id, Role, Timestamp};

fn generated_at(node: &Node) -> Timestamp {
    Timestamp::parse(&node.now_text()).expect("the node's time is RFC 3339")
}

/// The answer for a team that has no numbers: it is listed as unavailable, with the reason.
fn unavailable(
    node: &Node,
    hosted: &HostedTeam,
    period: &Period,
    days: u32,
    reason: &str,
) -> Effort {
    Effort {
        org: hosted.org.clone(),
        days,
        from: period.first(),
        until: period.last(),
        generated_at: generated_at(node),
        teams: vec![],
        unavailable: vec![TeamUnavailable {
            team: hosted.team.clone(),
            reason: reason.to_owned(),
        }],
    }
}

/// The effort of a team on this node over the last `days` days, as the answer of one team: the team in
/// `teams`, or in `unavailable` with the reason it has no numbers. A team whose runtime reports no token
/// usage has none: its effort is not zero, it is not known.
pub async fn effort_of_hosted(node: &Node, hosted: &HostedTeam, days: u32) -> Effort {
    let period = Period::new(node.now(), node.utc_offset(), days);
    let Some(source) = hosted.delivery_source() else {
        let reason = "no repositories are configured for this team";
        return unavailable(node, hosted, &period, days, reason);
    };
    let facts = match node.delivery().facts(&source, period.start()).await {
        Ok(facts) => facts,
        Err(DeliveryError(reason)) => return unavailable(node, hosted, &period, days, &reason),
    };
    let Ok(usage) = node.runtime().token_usage(hosted, days).await else {
        let reason = "the team's runtime does not report token usage";
        return unavailable(node, hosted, &period, days, reason);
    };
    let cost_by_day: Vec<_> = usage
        .by_day
        .iter()
        .map(|day| (day.date.clone(), day.cost_usd.values().sum()))
        .collect();
    let tokens = usage
        .by_person
        .iter()
        .fold(TokenCounts::default(), |total, person| TokenCounts {
            input: total.input.saturating_add(person.tokens.input),
            output: total.output.saturating_add(person.tokens.output),
            cache_read: total.cache_read.saturating_add(person.tokens.cache_read),
            cache_write: total.cache_write.saturating_add(person.tokens.cache_write),
        });
    let team = effort(
        hosted.team.clone(),
        &EffortInput {
            period,
            pull_requests: &facts.pull_requests,
            cost_by_day: &cost_by_day,
            tokens,
        },
    );
    Effort {
        org: hosted.org.clone(),
        days,
        from: period.first(),
        until: period.last(),
        generated_at: generated_at(node),
        teams: vec![team],
        unavailable: vec![],
    }
}

/// The effort of a team on another node, read live from it. Whatever goes wrong becomes the reason the
/// team is unavailable: no address and nothing the other node said is passed on.
async fn remote_effort(
    node: &Node,
    org: &Id,
    place: &TeamPlace,
    days: u32,
) -> std::result::Result<Effort, String> {
    let path = routes::effort(org, &place.id, days);
    let auth = sign_read(node.key(), &node.account(), "GET", &path, node.now());
    let answer = node
        .peers()
        .get_effort(&place.node, &path, &auth)
        .await
        .map_err(|error| match error {
            PeerError::Status { status: 404, .. } => {
                "the team's node does not give effort numbers".to_owned()
            }
            PeerError::Status { status: 403, .. } => {
                "the team's node does not give effort numbers to this account".to_owned()
            }
            _ => "the team's node did not answer".to_owned(),
        })?;
    let named: Vec<&Id> = answer
        .teams
        .iter()
        .map(|team| &team.team)
        .chain(answer.unavailable.iter().map(|team| &team.team))
        .collect();
    if answer.check(org, days).is_err() || named != [&place.id] {
        return Err("the team's node sent an answer that is not what was asked for".to_owned());
    }
    Ok(answer)
}

/// The effort of every team of an org for the node's account (Client API): a team on this node is worked
/// out here and a team elsewhere is read from its node. A team that cannot answer is listed in
/// `unavailable` with the reason, and the others are still answered. `days` defaults to 14. An answerer of
/// the org does not read it, as it does not read token usage on a team's node.
pub async fn org_effort(node: &Node, org: &Id, days: Option<u32>) -> Result<Effort> {
    let (doc, person) = membership(node, org)?;
    if person.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the org's effort",
        ));
    }
    let days = check_days(days.unwrap_or(DEFAULT_USAGE_DAYS))?;
    let period = Period::new(node.now(), node.utc_offset(), days);
    let mut answer = Effort {
        org: org.clone(),
        days,
        from: period.first(),
        until: period.last(),
        generated_at: generated_at(node),
        teams: vec![],
        unavailable: vec![],
    };
    for place in &doc.body.teams {
        let one = match node.hosted(org, &place.id) {
            Some(hosted) => Ok(effort_of_hosted(node, &hosted, days).await),
            None => remote_effort(node, org, place, days).await,
        };
        match one {
            Ok(one) => {
                answer.teams.extend(one.teams);
                answer.unavailable.extend(one.unavailable);
            }
            Err(reason) => answer.unavailable.push(TeamUnavailable {
                team: place.id.clone(),
                reason,
            }),
        }
    }
    Ok(answer)
}

/// The effort of a team on this node, for a signed read by a member of its org. The signature must cover
/// the path with this `days`. An answerer does not read it, as it does not read token usage.
pub async fn hosted_effort(
    node: &Node,
    org: &Id,
    team: &Id,
    days: u32,
    auth: &crate::read_auth::ReadAuth,
) -> Result<Effort> {
    let hosted = node.hosted(org, team);
    let doc = node.store().load_org(org);
    let reader = verify_read_once(
        node,
        doc.as_ref().map(|doc| &doc.body),
        hosted.is_some(),
        auth,
        "GET",
        &routes::effort(org, team, days),
    )?;
    let hosted =
        hosted.ok_or_else(|| unseen::refused("a read of a team that does not run here"))?;
    if reader.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the team's effort",
        ));
    }
    Ok(effort_of_hosted(node, &hosted, check_days(days)?).await)
}
