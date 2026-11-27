//! A team's weekly budget status, read from the usage of the teams that have a plan. The Client API
//! and the command line call the same function, so they cannot differ in the week or the thresholds.
use agency_core::budget::{definition_for_budget, status, today, week_spend, week_start};
use agency_core::error::Result;
use agency_core::node::Node;
use agency_core::usage::team_usage;
use agency_proto::budget::BudgetStatus;
use agency_proto::Id;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// The longest the usage of one team is waited for. A team whose usage does not come in time is
/// reported as `partial`, so that one slow node does not stop the answer for the others.
const MOST_WAIT_PER_TEAM: Duration = Duration::from_secs(8);

/// The budget status of `team` for the current week of the node's time zone: the team's own spend and
/// the org's, which is that of the teams that have a plan. A team whose usage cannot be read makes the
/// answer `partial` and is never counted as zero spent.
pub async fn budget_status(node: &Node, org: &Id, team: &Id) -> Result<BudgetStatus> {
    let definition = definition_for_budget(node, org, team)?;
    let (now, offset) = (node.now(), node.utc_offset());
    let (start, today) = (week_start(now, offset), today(now, offset));
    let mut teams: BTreeSet<Id> = definition
        .teams
        .iter()
        .filter(|place| place.plan.is_some())
        .map(|place| place.id.clone())
        .collect();
    teams.insert(team.clone());
    let reads = teams.into_iter().map(|id| {
        let start = &start;
        let today = &today;
        async move {
            let usage =
                tokio::time::timeout(MOST_WAIT_PER_TEAM, team_usage(node, org, &id, Some(7))).await;
            let spend = usage
                .ok()
                .and_then(|answer| answer.ok())
                .map(|usage| week_spend(&usage, start, today));
            (id, spend)
        }
    });
    let spends: BTreeMap<Id, Option<f64>> = futures_util::future::join_all(reads)
        .await
        .into_iter()
        .collect();
    Ok(status(&definition, start, team, &spends)
        .expect("the team was found in the definition just read"))
}
