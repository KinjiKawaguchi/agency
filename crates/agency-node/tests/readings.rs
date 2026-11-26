//! The readings of the numbers around the days epics shipped, through the Client API: owner-only like the
//! numbers themselves, with the ship days remembered by the node past what a team's view keeps. All figures
//! and names here are made up.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::ships::record_all;
use agency_core::{departments, kpis};
use agency_proto::org::{Allocation, TeamPlan};
use agency_proto::outcome::{GoodWhen, Kpi, KpiKind, KpiUnit};
use agency_proto::reading::{ReadingState, Readings};
use agency_proto::view::EpicForecast;
use agency_proto::Timestamp;
use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, body, get, id, post, Running, TOKEN};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const READINGS: &str = "/api/orgs/acme/outcomes/readings";
const OBSERVATIONS: &str = "/api/orgs/acme/outcomes/observations";

type Memory = Arc<Mutex<MemoryRuntimeState>>;

fn activation() -> Kpi {
    Kpi {
        id: id("activation"),
        label: "Activation".into(),
        kind: KpiKind::Product,
        unit: KpiUnit::Percent,
        good_when: GoodWhen::Higher,
    }
}

fn support_load() -> Kpi {
    Kpi {
        id: id("support-load"),
        label: "Tickets a week".into(),
        kind: KpiKind::Business,
        unit: KpiUnit::Count,
        good_when: GoodWhen::Lower,
    }
}

/// An epic of 3 children, `done` of them finished.
fn epic(reference: &str, done: u32, done_at: Option<&str>) -> EpicForecast {
    EpicForecast {
        total: 3,
        ..childless(reference, done, done_at)
    }
}

/// An epic with no children at all.
fn childless(reference: &str, done: u32, done_at: Option<&str>) -> EpicForecast {
    EpicForecast {
        reference: reference.into(),
        title: format!("Epic {reference}"),
        total: 0,
        done,
        doing: 0,
        remaining_points: 0,
        blocked: vec![],
        forecast: None,
        done_at: done_at.map(|at| Timestamp::parse(at).unwrap()),
        basis: "test".into(),
    }
}

/// The node's today is 2026-11-24 (its clock is at T0). The team `app` aims the given epics at activation.
async fn node_aiming(epics: &[&str]) -> (Running, Memory) {
    let (alice, memory) = alice().await;
    let (org, app) = (id("acme"), id("app"));
    kpis::add(&alice.node, &org, activation()).unwrap();
    let allocation = Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    };
    let plan = TeamPlan::new(allocation, 500);
    departments::plan_team(&alice.node, &org, &app, Some(plan)).unwrap();
    for reference in epics {
        let aim = kpis::aim(
            &alice.node,
            &org,
            &app,
            (*reference).into(),
            &id("activation"),
        );
        aim.unwrap();
    }
    (alice, memory)
}

async fn enter(running: &Running, date: &str, value: f64) {
    let input = json!({ "kpi": "activation", "date": date, "value": value });
    let request = post(running, OBSERVATIONS, &input).header("x-agency-token", TOKEN);
    assert_eq!(body(request.send().await.unwrap()).await.0, StatusCode::OK);
}

/// Three numbers around 2026-10-20: 40 before it, 42 ten days after, 44 on 11-10.
async fn enter_the_numbers(running: &Running) {
    enter(running, "2026-10-15", 40.0).await;
    enter(running, "2026-10-30", 42.0).await;
    enter(running, "2026-11-10", 44.0).await;
}

async fn readings(running: &Running) -> Readings {
    let (status, answer) = body(get(running, READINGS).send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let readings: Readings = serde_json::from_value(answer).unwrap();
    assert_eq!(readings.check(&id("acme")), Ok(()), "{readings:?}");
    readings
}

fn set_epics(memory: &Memory, epics: Vec<EpicForecast>) {
    memory.lock().unwrap().epics = epics;
}

fn ledger(running: &Running) -> std::path::PathBuf {
    running.data_dir().join("outcomes").join("acme.ships.jsonl")
}

#[tokio::test]
async fn a_finished_epic_is_read_against_the_numbers_around_its_ship_day() {
    let (alice, memory) = node_aiming(&["app#10", "app#12"]).await;
    enter_the_numbers(&alice).await;
    set_epics(
        &memory,
        vec![
            epic("app#10", 3, Some("2026-10-20T10:00:00Z")),
            epic("app#12", 3, Some("2026-10-25T10:00:00Z")),
        ],
    );
    // An epic aimed at another KPI that shipped in between is not named: it moves something else.
    kpis::add(&alice.node, &id("acme"), support_load()).unwrap();
    let aim = kpis::aim(
        &alice.node,
        &id("acme"),
        &id("app"),
        "app#13".into(),
        &id("support-load"),
    );
    aim.unwrap();
    memory
        .lock()
        .unwrap()
        .epics
        .push(epic("app#13", 3, Some("2026-10-22T10:00:00Z")));
    let answer = readings(&alice).await;
    assert_eq!(
        answer.readings.len(),
        3,
        "one for each aim, in the plan's order"
    );
    let first = &answer.readings[0];
    assert_eq!(
        (first.epic.as_str(), first.state),
        ("app#10", ReadingState::Ready)
    );
    assert_eq!(first.shipped_on.as_ref().unwrap().as_str(), "2026-10-20");
    assert_eq!(first.epic_title.as_deref(), Some("Epic app#10"));
    assert_eq!((first.change, first.change_ratio), (Some(4.0), Some(0.1)));
    assert_eq!(
        first.overlapping,
        ["app#12"],
        "shipped between the two numbers"
    );
    assert_eq!(answer.readings[1].overlapping, ["app#10"]);
}

#[tokio::test]
async fn the_ship_day_outlives_the_teams_view_and_keeps_its_first_time() {
    let (alice, memory) = node_aiming(&["app#10"]).await;
    enter_the_numbers(&alice).await;
    set_epics(
        &memory,
        vec![epic("app#10", 3, Some("2026-10-20T10:00:00Z"))],
    );
    let seen = readings(&alice).await;
    assert_eq!(seen.readings[0].state, ReadingState::Ready);

    // The team lets the finished epic go: the reading is the same, from the node's own ledger, except that
    // the title, which only the team's view has, is gone.
    set_epics(&memory, vec![]);
    let mut expected = seen.readings;
    expected[0].epic_title = None;
    assert_eq!(readings(&alice).await.readings, expected);

    // The epic is reopened and finished again later: the first time stays.
    set_epics(
        &memory,
        vec![epic("app#10", 3, Some("2026-11-02T10:00:00Z"))],
    );
    let again = readings(&alice).await;
    assert_eq!(
        again.readings[0].shipped_on.as_ref().unwrap().as_str(),
        "2026-10-20"
    );
    assert_eq!(
        std::fs::read_to_string(ledger(&alice))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[tokio::test]
async fn nothing_is_guessed_for_an_epic_that_is_not_finished_or_not_there() {
    let (alice, memory) = node_aiming(&["app#10", "app#11", "app#12", "app#13"]).await;
    enter_the_numbers(&alice).await;
    set_epics(
        &memory,
        vec![
            epic("app#10", 1, None),
            epic("app#11", 3, None),
            childless("app#12", 0, None),
        ],
    );
    let states: Vec<ReadingState> = readings(&alice)
        .await
        .readings
        .iter()
        .map(|one| one.state)
        .collect();
    assert_eq!(
        states,
        [
            ReadingState::NotShipped,
            ReadingState::ShipDateUnknown,
            ReadingState::NotShipped,
            ReadingState::ShipDateUnknown,
        ],
        "open, finished with no known time, no children, and not listed at all"
    );
    assert!(
        !ledger(&alice).exists(),
        "nothing was seen finished, so nothing is written"
    );
}

#[tokio::test]
async fn the_nodes_own_loop_remembers_a_ship_day_nobody_read() {
    let (alice, memory) = node_aiming(&["app#10"]).await;
    enter_the_numbers(&alice).await;
    set_epics(
        &memory,
        vec![epic("app#10", 3, Some("2026-10-20T10:00:00Z"))],
    );
    record_all(&alice.node).await.unwrap();
    record_all(&alice.node).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(ledger(&alice))
            .unwrap()
            .lines()
            .count(),
        1,
        "once"
    );

    set_epics(&memory, vec![]);
    assert_eq!(
        readings(&alice).await.readings[0].state,
        ReadingState::Ready
    );
}

#[tokio::test]
async fn the_ledger_is_a_private_file_with_no_number_in_it() {
    use std::os::unix::fs::PermissionsExt;
    let (alice, memory) = node_aiming(&["app#10"]).await;
    enter_the_numbers(&alice).await;
    set_epics(
        &memory,
        vec![epic("app#10", 3, Some("2026-10-20T10:00:00Z"))],
    );
    readings(&alice).await;
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(alice.data_dir().join("outcomes").as_path()), 0o700);
    assert_eq!(mode(&ledger(&alice)), 0o600);
    let text = std::fs::read_to_string(ledger(&alice)).unwrap();
    let line: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(
        line,
        json!({ "team": "app", "epic": "app#10", "shippedAt": "2026-10-20T10:00:00Z" })
    );
}

#[tokio::test]
async fn through_access_only_the_owner_reads_the_readings() {
    let running = behind_access(None).await;
    kpis::add(&running.node, &id("acme"), activation()).unwrap();
    let via_access = |email: &str| {
        reqwest::Client::new()
            .get(format!("{}{READINGS}", running.url()))
            .header("host", PUBLIC_HOST)
            .header("cf-access-jwt-assertion", access_token(email, |_| {}))
    };
    let (status, _) = body(via_access("alice@example.com").send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "the owner reads them");
    let (status, answer) = body(via_access("carol@example.com").send().await.unwrap()).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "someone else Access lets in does not: {answer}"
    );
    let (status, _) = body(
        get(&running, "/api/orgs/other/outcomes/readings")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an org this node is not in");
}

#[tokio::test]
async fn when_two_passes_wrote_the_same_epic_the_first_line_counts() {
    let (alice, memory) = node_aiming(&["app#10"]).await;
    enter_the_numbers(&alice).await;
    let lines = [
        r#"{"team":"app","epic":"app#10","shippedAt":"2026-10-20T10:00:00Z"}"#,
        r#"{"team":"app","epic":"app#10","shippedAt":"2026-11-02T10:00:00Z"}"#,
        "not a record",
    ];
    std::fs::create_dir_all(alice.data_dir().join("outcomes")).unwrap();
    std::fs::write(ledger(&alice), lines.join("\n") + "\n").unwrap();
    set_epics(&memory, vec![]);
    let answer = readings(&alice).await;
    assert_eq!(
        answer.readings[0].shipped_on.as_ref().unwrap().as_str(),
        "2026-10-20"
    );
}
