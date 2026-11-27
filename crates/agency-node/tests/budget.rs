//! Budget command data preparation: only costs inside the node's current calendar week count.
use agency_node::budget::week_spend;
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
