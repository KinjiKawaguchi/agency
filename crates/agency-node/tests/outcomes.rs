//! The numbers of the outcome analysis through the Client API: business data, so every route is owner-only
//! (a read as well as the write), the numbers live only in a file of the node's data directory, and nothing
//! else the node serves holds one. All figures here are made up.
mod common;

use agency_core::kpis;
use agency_proto::outcome::{Currency, GoodWhen, Kpi, KpiKind, KpiUnit};
use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, body, get, id, post, Running, TOKEN};
use reqwest::StatusCode;
use serde_json::{json, Value};

const OBSERVATIONS: &str = "/api/orgs/acme/outcomes/observations";
const OUTCOMES: &str = "/api/orgs/acme/outcomes";

fn revenue() -> Kpi {
    Kpi {
        id: id("revenue"),
        label: "Monthly revenue".into(),
        kind: KpiKind::Business,
        unit: KpiUnit::Money {
            currency: Currency::parse("JPY").unwrap(),
        },
        good_when: GoodWhen::Higher,
    }
}

fn activation() -> Kpi {
    Kpi {
        id: id("activation"),
        label: "Activation".into(),
        kind: KpiKind::Product,
        unit: KpiUnit::Percent,
        good_when: GoodWhen::Higher,
    }
}

async fn node_with_kpis() -> Running {
    let (alice, _) = alice().await;
    kpis::add(&alice.node, &id("acme"), revenue()).unwrap();
    kpis::add(&alice.node, &id("acme"), activation()).unwrap();
    alice
}

async fn enter(running: &Running, input: Value) -> (StatusCode, Value) {
    let request = post(running, OBSERVATIONS, &input).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await
}

async fn read(running: &Running, path: &str) -> (StatusCode, Value) {
    body(get(running, path).send().await.unwrap()).await
}

/// 2026-11-24 is the node's today in these tests (its clock is at T0).
fn number(kpi: &str, date: &str, value: f64) -> Value {
    json!({ "kpi": kpi, "date": date, "value": value })
}

#[tokio::test]
async fn the_owner_enters_a_number_and_the_node_says_when_and_by_whom() {
    let alice = node_with_kpis().await;
    let (status, saved) = enter(&alice, number("revenue", "2026-11-20", 1_350_000.0)).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["enteredAt"], "2026-11-24T12:00:00Z");
    assert_eq!(saved["enteredBy"], "alice");
    assert_eq!(saved["source"], json!({ "kind": "manual" }));

    let mut named = number("activation", "2026-11-17", 41.5);
    named["connector"] = json!("analytics-export");
    named["note"] = json!("from the export");
    let (_, saved) = enter(&alice, named).await;
    assert_eq!(
        saved["source"],
        json!({ "kind": "connector", "name": "analytics-export" })
    );
    assert_eq!(saved["note"], "from the export");

    for field in ["enteredAt", "enteredBy", "source"] {
        let mut input = number("revenue", "2026-11-21", 1.0);
        input[field] = json!("x");
        assert_eq!(
            enter(&alice, input).await.0,
            StatusCode::BAD_REQUEST,
            "{field} is the node's"
        );
    }
}

#[tokio::test]
async fn a_number_must_be_for_a_kpi_of_the_org_and_well_formed() {
    let alice = node_with_kpis().await;
    assert_eq!(
        enter(&alice, number("churn", "2026-11-20", 1.0)).await.0,
        StatusCode::NOT_FOUND
    );
    let refused = [
        (
            "a date more than a day ahead",
            number("revenue", "2026-11-26", 1.0),
        ),
        (
            "a date older than ten years",
            number("revenue", "2016-11-20", 1.0),
        ),
        ("a value too large", number("revenue", "2026-11-20", 1.5e15)),
        (
            "a note with a control character",
            json!({ "kpi": "revenue", "date": "2026-11-20", "value": 1, "note": "a\nb" }),
        ),
        (
            "a note that is too long",
            json!({ "kpi": "revenue", "date": "2026-11-20", "value": 1, "note": "n".repeat(201) }),
        ),
        (
            "an empty connector",
            json!({ "kpi": "revenue", "date": "2026-11-20", "value": 1, "connector": "" }),
        ),
        ("a malformed date", number("revenue", "yesterday", 1.0)),
    ];
    for (what, input) in refused {
        assert_eq!(
            enter(&alice, input).await.0,
            StatusCode::BAD_REQUEST,
            "{what}"
        );
    }
    assert_eq!(
        enter(&alice, number("revenue", "2026-11-25", 1.0)).await.0,
        StatusCode::OK,
        "a day ahead is allowed"
    );
    assert_eq!(
        enter(&alice, number("revenue", "2016-11-24", -1e15))
            .await
            .0,
        StatusCode::OK,
        "ten years back, a loss"
    );
}

#[tokio::test]
async fn a_second_entry_for_a_day_wins_and_the_first_stays_in_the_history() {
    let alice = node_with_kpis().await;
    enter(&alice, number("revenue", "2026-11-20", 100.0)).await;
    alice
        .clock
        .advance(agency_proto::clock::Duration::seconds(60));
    enter(&alice, number("revenue", "2026-11-20", 120.0)).await;

    let (_, outcomes) = read(&alice, OUTCOMES).await;
    let revenue = &outcomes["series"][0];
    assert_eq!(revenue["points"].as_array().unwrap().len(), 1);
    assert_eq!(
        revenue["points"][0]["value"], 120.0,
        "the later entry counts"
    );

    let (_, history) = read(&alice, &format!("{OBSERVATIONS}?kpi=revenue")).await;
    let values: Vec<&Value> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|one| &one["value"])
        .collect();
    assert_eq!(
        values,
        [&json!(120.0), &json!(100.0)],
        "newest first, the first one still there"
    );
    let (_, limited) = read(&alice, &format!("{OBSERVATIONS}?kpi=revenue&limit=1")).await;
    assert_eq!(limited.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn the_series_hold_the_period_oldest_first_and_the_baseline_before_it() {
    let alice = node_with_kpis().await;
    for (date, value) in [
        ("2026-10-31", 1_200_000.0),
        ("2026-11-24", 1_400_000.0),
        ("2026-11-20", 1_350_000.0),
        ("2026-10-15", 1_000_000.0),
    ] {
        enter(&alice, number("revenue", date, value)).await;
    }
    let (status, outcomes) = read(&alice, &format!("{OUTCOMES}?days=14")).await;
    assert_eq!(status, StatusCode::OK, "{outcomes}");
    assert_eq!(
        (outcomes["from"].as_str(), outcomes["until"].as_str()),
        (Some("2026-11-11"), Some("2026-11-24"))
    );
    let ids: Vec<&str> = outcomes["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["kpi"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["revenue", "activation"],
        "every KPI of the definition, in its order"
    );
    let revenue = &outcomes["series"][0];
    let dates: Vec<&str> = revenue["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["date"].as_str().unwrap())
        .collect();
    assert_eq!(
        dates,
        ["2026-11-20", "2026-11-24"],
        "oldest first, only the period"
    );
    assert_eq!(
        revenue["before"]["date"], "2026-10-31",
        "the last number before the period"
    );
    assert_eq!(outcomes["series"][1]["points"], json!([]));
    assert!(
        outcomes["series"][1].get("before").is_none(),
        "no baseline is left out"
    );

    assert_eq!(
        read(&alice, &format!("{OUTCOMES}?days=0")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read(&alice, &format!("{OUTCOMES}?days=many")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read(&alice, &format!("{OBSERVATIONS}?kpi=revenue&limit=501"))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read(&alice, &format!("{OBSERVATIONS}?kpi=nope")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        read(&alice, OBSERVATIONS).await.0,
        StatusCode::BAD_REQUEST,
        "a history names its KPI"
    );
}

#[tokio::test]
async fn through_access_only_the_owner_reads_or_enters_the_numbers() {
    let running = behind_access(None).await;
    kpis::add(&running.node, &id("acme"), revenue()).unwrap();
    let via_access = |email: &str, method: &str, path: &str, input: Option<Value>| {
        let url = format!("{}{path}", running.url());
        let client = reqwest::Client::new();
        let request = match method {
            "POST" => client
                .post(url)
                .json(&input.unwrap())
                .header("x-agency-token", TOKEN),
            _ => client.get(url),
        };
        request
            .header("host", PUBLIC_HOST)
            .header("cf-access-jwt-assertion", access_token(email, |_| {}))
    };
    let owner = "alice@example.com";
    let other = "carol@example.com";
    let input = || Some(number("revenue", "2026-11-20", 5.0));

    let (status, _) = body(
        via_access(owner, "POST", OBSERVATIONS, input())
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the owner enters a number");
    for path in [OUTCOMES, &format!("{OBSERVATIONS}?kpi=revenue")] {
        let (status, answer) =
            body(via_access(owner, "GET", path, None).send().await.unwrap()).await;
        assert_eq!(status, StatusCode::OK, "the owner reads {path}: {answer}");
    }

    // Someone Access lets in who is not the owner reads the rest of the API, but none of the numbers.
    let (status, _) = body(
        via_access(other, "GET", "/api/me", None)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Access decides who may read the rest"
    );
    for (method, path) in [
        ("GET", OUTCOMES.to_owned()),
        ("GET", format!("{OBSERVATIONS}?kpi=revenue")),
        ("POST", OBSERVATIONS.to_owned()),
    ] {
        let (status, answer) = body(
            via_access(other, method, &path, input())
                .send()
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {path} for a signed-in email that is not the owner's"
        );
        assert!(
            !answer.to_string().contains("5.0"),
            "no number in a refusal"
        );
    }
    let (_, history) = body(
        via_access(owner, "GET", &format!("{OBSERVATIONS}?kpi=revenue"), None)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        history.as_array().unwrap().len(),
        1,
        "the refused POST entered nothing"
    );
}

#[tokio::test]
async fn a_kpi_keeps_at_most_5000_numbers() {
    let alice = node_with_kpis().await;
    let org = id("acme");
    let observation = |n: usize| agency_proto::observation::Observation {
        kpi: id("revenue"),
        date: agency_proto::Date::parse("2026-11-20").unwrap(),
        value: n as f64,
        source: agency_proto::observation::Source::Manual,
        entered_at: agency_proto::Timestamp::parse("2026-11-24T12:00:00Z").unwrap(),
        entered_by: id("alice"),
        note: None,
    };
    // The file is written at once: 5,000 appends, each synced, would take long for what this checks.
    let lines: Vec<String> = (0..5_000)
        .map(|n| serde_json::to_string(&observation(n)).unwrap())
        .collect();
    alice
        .node
        .store()
        .append_observation(&org, &observation(0))
        .unwrap();
    let file = alice.data_dir().join("outcomes").join("acme.jsonl");
    std::fs::write(&file, lines.join("\n") + "\n").unwrap();
    let (status, _) = enter(&alice, number("revenue", "2026-11-21", 1.0)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = enter(&alice, number("activation", "2026-11-21", 1.0)).await;
    assert_eq!(status, StatusCode::OK, "the limit is for each KPI");
}

#[tokio::test]
async fn the_numbers_are_in_a_private_file_and_nowhere_else_the_node_serves() {
    use std::os::unix::fs::PermissionsExt;
    let alice = node_with_kpis().await;
    enter(&alice, number("revenue", "2026-11-20", 1_234_567.0)).await;

    // Nothing else the node serves holds the value: the definition, the evaluation, the Client API's other answers.
    for path in [
        "/api/orgs/acme",
        "/api/me",
        "/api/decisions",
        "/api/devices",
        "/api/orgs/acme/teams/app/view",
    ] {
        let (_, answer) = read(&alice, path).await;
        assert!(!answer.to_string().contains("1234567"), "{path}");
    }
    let definition =
        serde_json::to_string(&alice.node.store().load_org(&id("acme")).unwrap()).unwrap();
    assert!(
        !definition.contains("1234567"),
        "the signed definition holds no number"
    );

    let dir = alice.data_dir().join("outcomes");
    let file = dir.join("acme.jsonl");
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text.lines().count(), 1, "one line for each number");
    assert!(text.contains("1234567"));
}
