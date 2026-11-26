//! A team's delivery evaluation (docs/protocol.md, "Evaluation"): worked out on the node the team runs on
//! from its repositories on GitHub and its token usage, and passed on to other nodes over a signed read.
//! Nothing is stored here; a read of GitHub is kept for a short while by the adapter, not by this module.
use crate::config::HostedTeam;
use crate::delivery::{evaluate, Input, Period};
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::ports::DeliveryError;
use crate::read_auth::{verify_read_once, ReadAuth};
use crate::routes;
use crate::unseen;
use crate::usage::check_days;
use agency_proto::evaluation::{Evaluation, TeamUnavailable};
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

/// The repositories of a hosted team, if its configuration names any.
fn repositories(hosted: &HostedTeam) -> Option<Vec<String>> {
    hosted
        .delivery
        .as_ref()
        .map(|delivery| delivery.repos.clone())
        .filter(|repos| !repos.is_empty())
}

/// The evaluation of a team on this node over the last `days` days, as the answer of one team: the team
/// in `teams`, or in `unavailable` with the reason it has no numbers. It does not fail: a team that cannot
/// be evaluated is a result that a screen shows, not an error that blanks it.
pub async fn evaluate_hosted(node: &Node, hosted: &HostedTeam, days: u32) -> Evaluation {
    let now = node.now();
    let period = Period::new(now, node.utc_offset(), days);
    let Some(repos) = repositories(hosted) else {
        let reason = "no repositories are configured for this team".to_owned();
        return unavailable(node, hosted, &period, days, reason);
    };
    let facts = match node.delivery().facts(&repos, period.start()).await {
        Ok(facts) => facts,
        Err(DeliveryError(reason)) => return unavailable(node, hosted, &period, days, reason),
    };
    let costs = daily_costs(node, hosted, days).await;
    let input = Input {
        now,
        offset: node.utc_offset(),
        days,
        pull_requests: &facts.pull_requests,
        runs: &facts.runs,
        cost_by_day: &costs,
    };
    Evaluation {
        org: hosted.org.clone(),
        days,
        from: period.first(),
        until: period.last(),
        generated_at: generated_at(node),
        teams: vec![evaluate(hosted.team.clone(), &input)],
        unavailable: vec![],
    }
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
    Ok(evaluate_hosted(node, &hosted, check_days(days)?).await)
}
