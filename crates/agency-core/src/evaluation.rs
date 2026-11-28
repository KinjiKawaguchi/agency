//! A team's delivery evaluation (docs/protocol.md, "Evaluation"): worked out on the node the team runs on
//! from its repositories on GitHub and its token usage, and passed on to other nodes over a signed read.
//! Nothing is stored here; a read of GitHub is kept for a short while by the adapter, not by this module.
use crate::client::membership;
use crate::config::HostedTeam;
use crate::delivery::{attribute, evaluate, of_team, Input, Period};
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::ports::{DeliveryError, PeerError};
use crate::read_auth::{sign_for_peer, verify_read_once, ReadAuth};
use crate::routes;
use crate::unseen;
use crate::usage::check_days;
use agency_proto::evaluation::{Evaluation, TeamUnavailable};
use agency_proto::org::TeamPlace;
use agency_proto::participant_metrics::ParticipantMetrics;
use agency_proto::usage::DEFAULT_USAGE_DAYS;
use agency_proto::{Date, Id, Role, Timestamp};

/// The answer for a team that has no numbers: it is listed as unavailable, with the reason.
fn unavailable(
    node: &Node,
    hosted: &HostedTeam,
    period: &Period,
    days: u32,
    reason: String,
) -> Evaluation {
    Evaluation {
        org: hosted.org.clone(),
        days,
        from: period.first(),
        until: period.last(),
        generated_at: generated_at(node),
        teams: vec![],
        unavailable: vec![TeamUnavailable {
            team: hosted.team.clone(),
            reason,
        }],
    }
}

fn generated_at(node: &Node) -> Timestamp {
    Timestamp::parse(&node.now_text()).expect("the node's time is RFC 3339")
}

/// The cost of each day's tokens, if the team's runtime reports usage. Without it the costs are zero:
/// delivery numbers do not wait for them.
async fn daily_costs(node: &Node, hosted: &HostedTeam, days: u32) -> Vec<(Date, f64)> {
    let Ok(usage) = node.runtime().token_usage(hosted, days).await else {
        return vec![];
    };
    usage
        .by_day
        .into_iter()
        .map(|day| (day.date, day.cost_usd.values().sum()))
        .collect()
}

/// The team's participants over the period, if its runtime reports them and what it reports is what was
/// asked for (the team, the range, and the node's time zone, so that its days are the evaluation's days).
/// Without it the numbers are the same as before: they do not wait for participants.
async fn participants_of(
    node: &Node,
    hosted: &HostedTeam,
    days: u32,
    offset_minutes: i32,
) -> Option<ParticipantMetrics> {
    let report = node
        .runtime()
        .participant_metrics(hosted, days, offset_minutes)
        .await
        .ok()?;
    report
        .check(&hosted.org, &hosted.team, days, Some(offset_minutes))
        .ok()?;
    Some(report)
}

/// The evaluation of a team on this node over the last `days` days, as the answer of one team: the team
/// in `teams`, or in `unavailable` with the reason it has no numbers. It does not fail: a team that cannot
/// be evaluated is a result that a screen shows, not an error that blanks it. What each participant did
/// is the team owner's to read: it is worked out, and runs the team's runtime, only when `reader` (an org
/// member id) is the team's owner.
pub async fn evaluate_hosted(
    node: &Node,
    hosted: &HostedTeam,
    days: u32,
    reader: &Id,
) -> Evaluation {
    let now = node.now();
    let period = Period::new(now, node.utc_offset(), days);
    let Some(source) = hosted.delivery_source() else {
        let reason = "no repositories are configured for this team".to_owned();
        return unavailable(node, hosted, &period, days, reason);
    };
    let facts = match node.delivery().facts(&source, period.start()).await {
        Ok(facts) => facts,
        Err(DeliveryError(reason)) => return unavailable(node, hosted, &period, days, reason),
    };
    let pull_requests = of_team(hosted.delivery.as_ref(), facts.pull_requests);
    let costs = daily_costs(node, hosted, days).await;
    let offset_minutes = i32::from(node.utc_offset().whole_minutes());
    let participants = match reader == &hosted.owner {
        true => participants_of(node, hosted, days, offset_minutes).await,
        false => None,
    };
    let input = Input {
        now,
        offset: node.utc_offset(),
        days,
        pull_requests: &pull_requests,
        runs: &facts.runs,
        cost_by_day: &costs,
    };
    let mut team = evaluate(hosted.team.clone(), &input);
    if let Some(report) = &participants {
        (team.participants, team.unattributed_merged) = attribute(report, &pull_requests, &period);
    }
    Evaluation {
        org: hosted.org.clone(),
        days,
        from: period.first(),
        until: period.last(),
        generated_at: generated_at(node),
        teams: vec![team],
        unavailable: vec![],
    }
}

/// The evaluation of a team on another node, read live from it. Whatever goes wrong becomes the reason
/// the team is unavailable: no address and nothing the other node said is passed on.
async fn remote_evaluation(
    node: &Node,
    org: &Id,
    place: &TeamPlace,
    days: u32,
) -> std::result::Result<Evaluation, String> {
    let path = routes::evaluation(org, &place.id, days);
    let auth = sign_for_peer(node, org, &place.node, &path);
    let answer = node
        .peers()
        .get_evaluation(&place.node, &path, &auth)
        .await
        .map_err(|error| match error {
            PeerError::Status { status: 404, .. } => {
                "the team's node does not give delivery numbers".to_owned()
            }
            PeerError::Status { status: 403, .. } => {
                "the team's node does not give delivery numbers to this account".to_owned()
            }
            _ => "the team's node did not answer".to_owned(),
        })?;
    let names_the_team = |teams: &[Id]| teams.len() == 1 && teams[0] == place.id;
    let named: Vec<Id> = answer
        .teams
        .iter()
        .map(|team| team.team.clone())
        .chain(answer.unavailable.iter().map(|team| team.team.clone()))
        .collect();
    if answer.check(org, days).is_err() || !names_the_team(&named) {
        return Err("the team's node sent an answer that is not what was asked for".to_owned());
    }
    Ok(answer)
}

/// The evaluation of every team of an org for the node's account (Client API): a team on this node is
/// worked out here and a team elsewhere is read from its node. A team that cannot answer is listed in
/// `unavailable` with the reason, and the others are still answered. `days` defaults to 14. An answerer
/// of the org does not read it, as it does not read token usage on a team's node.
pub async fn org_evaluation(node: &Node, org: &Id, days: Option<u32>) -> Result<Evaluation> {
    let (doc, person) = membership(node, org)?;
    if person.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the org's evaluation",
        ));
    }
    let days = check_days(days.unwrap_or(DEFAULT_USAGE_DAYS))?;
    let period = Period::new(node.now(), node.utc_offset(), days);
    let mut answer = Evaluation {
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
            Some(hosted) => Ok(evaluate_hosted(node, &hosted, days, &person.id).await),
            None => remote_evaluation(node, org, place, days).await,
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

/// The evaluation of a team on this node, for a signed read by a member of its org. The signature must
/// cover the path with this `days`. An answerer does not read it, as it does not read token usage.
pub async fn hosted_evaluation(
    node: &Node,
    org: &Id,
    team: &Id,
    days: u32,
    auth: &ReadAuth,
) -> Result<Evaluation> {
    let hosted = node.hosted(org, team);
    let doc = node.store().load_org(org);
    let reader = verify_read_once(
        node,
        doc.as_ref().map(|doc| &doc.body),
        hosted.is_some(),
        auth,
        "GET",
        &routes::evaluation(org, team, days),
    )?;
    let hosted =
        hosted.ok_or_else(|| unseen::refused("a read of a team that does not run here"))?;
    if reader.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the team's evaluation",
        ));
    }
    Ok(evaluate_hosted(node, &hosted, check_days(days)?, &reader.id).await)
}
