//! Budget command data preparation: only costs inside the node's current calendar week count.
use agency_core::budget::week_spend;
use agency_node::budget::json_line;
use agency_proto::budget::{BudgetPart, BudgetState, BudgetStatus, OrgBudget};
use agency_proto::{Date, TokenUsage};

fn usage() -> TokenUsage {
    serde_json::from_value(serde_json::json!({
        "org": "acme",
        "team": "app",
        "days": 7,
        "generatedAt": "2026-11-24T12:00:00Z",
        "byDay": [
            { "date": "2026-11-22", "costUsd": { "agent": 9.0 } },
            { "date": "2026-11-23", "costUsd": { "agent": 2.5, "person": 1.5 } },
            { "date": "2026-11-24", "costUsd": { "agent": 3.0 } },
            { "date": "2026-11-25", "costUsd": { "agent": 100.0 } }
        ],
        "byPerson": []
    }))
    .unwrap()
}

#[test]
fn uses_only_days_in_the_monday_based_week_through_today() {
    let start = Date::parse("2026-11-23").unwrap();
    let today = Date::parse("2026-11-24").unwrap();
    assert_eq!(week_spend(&usage(), &start, &today), 7.0);
}

fn status(team: BudgetPart, org: OrgBudget) -> BudgetStatus {
    BudgetStatus {
        week_start: Date::parse("2026-11-23").unwrap(),
        team,
        org,
    }
}

#[test]
fn prints_the_harness_golden_json_cases() {
    let warning = status(
        BudgetPart {
            cap_usd: Some(200),
            spent_usd: 163.2,
            state: BudgetState::Warning,
            partial: false,
        },
        OrgBudget {
            cap_usd: Some(500),
            spent_usd: 311.0,
            state: BudgetState::Ok,
            partial: false,
        },
    );
    assert_eq!(
        json_line(&warning).unwrap(),
        r#"{"weekStart":"2026-11-23","team":{"capUsd":200,"spentUsd":163.2,"state":"warning"},"org":{"capUsd":500,"spentUsd":311.0,"state":"ok"}}"#
    );

    let org_over = status(
        BudgetPart {
            cap_usd: Some(200),
            spent_usd: 50.0,
            state: BudgetState::Over,
            partial: false,
        },
        OrgBudget {
            cap_usd: Some(500),
            spent_usd: 520.5,
            state: BudgetState::Over,
            partial: true,
        },
    );
    assert_eq!(
        json_line(&org_over).unwrap(),
        r#"{"weekStart":"2026-11-23","team":{"capUsd":200,"spentUsd":50.0,"state":"over"},"org":{"capUsd":500,"spentUsd":520.5,"state":"over","partial":true}}"#
    );

    let unplanned = status(
        BudgetPart {
            cap_usd: None,
            spent_usd: 7.0,
            state: BudgetState::Ok,
            partial: false,
        },
        OrgBudget {
            cap_usd: None,
            spent_usd: 0.0,
            state: BudgetState::Ok,
            partial: false,
        },
    );
    assert_eq!(
        json_line(&unplanned).unwrap(),
        r#"{"weekStart":"2026-11-23","team":{"spentUsd":7.0,"state":"ok"},"org":{"spentUsd":0.0,"state":"ok"}}"#
    );
}
