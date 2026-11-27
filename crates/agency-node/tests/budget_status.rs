//! A team's weekly budget status, read from the usage of the teams that have a plan: the week is the
//! node's Monday-based week, a team whose usage cannot be read is `partial` and never zero, and the
//! Client API and the command line read the same function.
mod common;

use agency_adapters::budget::budget_status;
use agency_core::budget::BudgetState;
use agency_core::departments::plan_team;
use agency_core::error::ErrorKind;
use agency_core::invites::{invite, join};
use agency_core::orgs;
use agency_proto::org::{Allocation, TeamPlace, TeamPlan};
use agency_proto::{Id, Role};
use common::{alice, id, joined_bob, node, Running};

fn plan(weekly_budget_usd: u32) -> TeamPlan {
    TeamPlan {
        aims: vec![],
        allocation: Allocation {
            new_feature: 50,
            improvement: 20,
            maintenance: 20,
            debt: 10,
            incident: 0,
        },
        weekly_budget_usd,
    }
}

/// The usage of the memory runtime: costs on the Sunday before the week (not counted), the Monday and the
/// Tuesday of the node's time (2026-11-24T12:00Z, a Tuesday), and a Wednesday that cannot have happened yet.
fn usage(memory: &std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>) {
    memory.lock().unwrap().usage = Some(
        serde_json::from_value(serde_json::json!({
            "org": "acme", "team": "app", "days": 7, "generatedAt": "2026-11-24T12:00:00Z",
            "byDay": [
                { "date": "2026-11-22", "costUsd": { "agent": 90.0 } },
                { "date": "2026-11-23", "costUsd": { "agent": 100.0, "person": 20.0 } },
                { "date": "2026-11-24", "costUsd": { "agent": 50.0 } },
                { "date": "2026-11-25", "costUsd": { "agent": 1000.0 } }
            ],
            "byPerson": []
        }))
        .unwrap(),
    );
}

fn app() -> Id {
    id("app")
}

async fn with_plan(
    budget: u32,
) -> (
    Running,
    std::sync::Arc<std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>>,
) {
    let (alice, memory) = alice().await;
    plan_team(&alice.node, &id("acme"), &app(), Some(plan(budget))).unwrap();
    usage(&memory);
    (alice, memory)
}

#[tokio::test]
async fn only_the_days_of_this_week_through_today_are_spent() {
    let (alice, _) = with_plan(200).await;
    let status = budget_status(&alice.node, &id("acme"), &app())
        .await
        .unwrap();
    assert_eq!(
        status.week_start.as_str(),
        "2026-11-23",
        "the Monday before the Tuesday"
    );
    assert_eq!(
        status.team.spent_usd, 170.0,
        "not the Sunday, not the day after today"
    );
    assert_eq!(status.team.cap_usd, Some(200));
    assert_eq!(
        status.team.state,
        BudgetState::Warning,
        "170 of 200 is past 80%"
    );
    assert!(!status.team.partial && !status.org.partial);
    assert_eq!(
        (status.org.cap_usd, status.org.spent_usd),
        (Some(200), 170.0)
    );
}

#[tokio::test]
async fn a_team_at_its_cap_is_over_and_one_below_80_percent_is_ok() {
    let (alice, _) = with_plan(170).await;
    let at_cap = budget_status(&alice.node, &id("acme"), &app())
        .await
        .unwrap();
    assert_eq!(
        at_cap.team.state,
        BudgetState::Over,
        "spent equal to the cap is over"
    );
    plan_team(&alice.node, &id("acme"), &app(), Some(plan(213))).unwrap();
    let below = budget_status(&alice.node, &id("acme"), &app())
        .await
        .unwrap();
    assert_eq!(below.team.state, BudgetState::Ok, "170 is 79.8% of 213");
    plan_team(&alice.node, &id("acme"), &app(), None).unwrap();
    let none = budget_status(&alice.node, &id("acme"), &app())
        .await
        .unwrap();
    assert_eq!(
        (none.team.cap_usd, none.team.state),
        (None, BudgetState::Ok)
    );
    assert_eq!(none.org.cap_usd, None, "no team has a plan");
    assert_eq!(
        none.team.spent_usd, 170.0,
        "the spend is known without a plan"
    );
}

#[tokio::test]
async fn a_team_whose_usage_cannot_be_read_is_partial_and_not_a_number() {
    let (alice, memory) = with_plan(200).await;
    memory.lock().unwrap().usage = None;
    let status = budget_status(&alice.node, &id("acme"), &app())
        .await
        .unwrap();
    assert!(status.team.partial, "the runtime reports no usage");
    assert!(
        status.org.partial,
        "so the org's spend is a lower bound too"
    );
    assert_eq!(
        status.team.spent_usd, 0.0,
        "the number is there, and partial says it is no knowledge"
    );
}

#[tokio::test]
async fn a_planned_team_elsewhere_that_does_not_answer_makes_only_the_org_partial() {
    let (alice, _) = with_plan(200).await;
    // site runs on bob's node, which lists the capabilities a plan needs; then bob goes down.
    let mut bob = joined_bob(&alice).await;
    agency_core::peer_about::read_origin(&alice.node, &id("acme"), &bob.url())
        .await
        .unwrap();
    let site = TeamPlace::new(id("site"), "site", bob.url());
    orgs::add_team(&alice.node, &id("acme"), site).unwrap();
    plan_team(&alice.node, &id("acme"), &id("site"), Some(plan(300))).unwrap();
    bob.stop().await;
    let status = budget_status(&alice.node, &id("acme"), &app())
        .await
        .unwrap();
    assert!(!status.team.partial, "app's own usage is read");
    assert!(status.org.partial, "site's is not");
    assert_eq!(
        (status.org.cap_usd, status.org.spent_usd),
        (Some(500), 170.0)
    );
}

#[tokio::test]
async fn an_unknown_team_and_an_answerer_read_nothing() {
    let (alice, _) = with_plan(200).await;
    let refused = budget_status(&alice.node, &id("acme"), &id("nobody"))
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::NotFound);
    let refused = budget_status(&alice.node, &id("other"), &app())
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::NotFound);

    let carol = node("carol", common::memory_runtime().0).await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let refused = budget_status(&carol.node, &id("acme"), &app())
        .await
        .unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Forbidden,
        "an answerer reads no usage"
    );
}
