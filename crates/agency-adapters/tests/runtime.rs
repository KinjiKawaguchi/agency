//! The command runtime against a fake agency-runtime/1 runtime: what it asks the runtime and what it makes of the
//! answers (a view, a decision, token usage, participants, activity), and how it times each decision.
mod common;

use agency_adapters::runtime::CommandRuntime;
use agency_core::config::RuntimeCommand;
use agency_core::ports::TeamRuntime;
use common::*;

#[tokio::test]
async fn views_and_decides_over_stdin_and_stdout() {
    let fake = Fake::new();
    let runtimes = CommandRuntime::default();
    let team = fake.team("app");

    let view = runtimes.view(&team).await.unwrap();
    assert_eq!(view.requests.len(), 3);
    runtimes
        .decide(&team, decision("--not-a-flag"))
        .await
        .unwrap();

    assert_eq!(
        fake.read("calls"),
        "describe\nview\ndecide\n",
        "describe once per team, before anything else"
    );
    let request: serde_json::Value = serde_json::from_str(&fake.read("last-decide.json")).unwrap();
    assert_eq!(request["runtime"], "agency-runtime/1");
    assert_eq!(
        (request["org"].as_str(), request["team"].as_str()),
        (Some("acme"), Some("app"))
    );
    assert_eq!(
        request["decision"]["payload"]["text"], "--not-a-flag",
        "decision text travels on stdin"
    );
}

#[tokio::test]
async fn refuses_a_runtime_that_does_not_speak_agency_runtime_1() {
    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/2"],"ops":["view"],"label":"App"}}"#,
        0,
    );
    let error = CommandRuntime::default()
        .view(&fake.team("app"))
        .await
        .unwrap_err();
    assert_eq!(error.status, 503);
    assert!(
        !fake.read("calls").contains("view"),
        "no view call to a runtime of another version"
    );
}

#[tokio::test]
async fn refuses_a_view_of_another_team() {
    let fake = Fake::new();
    let error = CommandRuntime::default()
        .view(&fake.team("other"))
        .await
        .unwrap_err();
    assert_eq!(error.status, 503);
    assert!(error.message.contains("acme/app"), "{}", error.message);
}

#[tokio::test]
async fn passes_runtime_errors_through_and_treats_garbage_as_unavailable() {
    let fake = Fake::new();
    let runtimes = CommandRuntime::default();
    let team = fake.team("app");
    fake.answer(
        "decide",
        r#"{"error":{"status":404,"message":"no such request"}}"#,
        1,
    );
    let error = runtimes.decide(&team, decision("")).await.unwrap_err();
    assert_eq!(
        (error.status, error.message.as_str()),
        (404, "no such request")
    );

    fake.answer("decide", "Traceback: something broke", 1);
    assert_eq!(
        runtimes
            .decide(&team, decision(""))
            .await
            .unwrap_err()
            .status,
        503
    );

    let missing = team.with_runtime(RuntimeCommand::of(vec![fake
        .path()
        .join("missing")
        .display()
        .to_string()]));
    assert_eq!(
        CommandRuntime::default()
            .view(&missing)
            .await
            .unwrap_err()
            .status,
        503
    );
}

#[tokio::test]
async fn reads_token_usage_only_from_a_runtime_that_lists_it() {
    let fake = Fake::new();
    let team = fake.team("app");
    let error = CommandRuntime::default()
        .token_usage(&team, 14)
        .await
        .unwrap_err();
    assert_eq!(error.status, 404);
    assert_eq!(
        fake.read("calls"),
        "describe\n",
        "no tokenUsage call to a runtime that does not list it"
    );

    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","tokenUsage"],"label":"App"}}"#,
        0,
    );
    fake.answer("tokenUsage", &format!(r#"{{"result":{USAGE}}}"#), 0);
    let usage = CommandRuntime::default()
        .token_usage(&fake.team("app"), 14)
        .await
        .unwrap();
    assert_eq!(usage.by_person.len(), 3);
    assert_eq!(fake.read("calls"), "describe\ntokenUsage\n");
    let request: serde_json::Value =
        serde_json::from_str(&fake.read("last-tokenUsage.json")).unwrap();
    assert_eq!(request["days"], 14);
}

#[tokio::test]
async fn participant_metrics_is_asked_with_the_range_and_time_zone_and_kept_for_the_same_ones() {
    let fake = Fake::new();
    let team = fake.team("app");
    // A runtime that does not list the op is not run for it.
    let error = CommandRuntime::default()
        .participant_metrics(&team, 2, 540)
        .await
        .unwrap_err();
    assert_eq!(error.status, 404);
    assert!(!fake.read("calls").contains("participantMetrics"));

    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","participantMetrics"],"label":"App"}}"#,
        0,
    );
    fake.answer(
        "participantMetrics",
        &format!(r#"{{"result":{PARTICIPANTS}}}"#),
        0,
    );
    let runtimes = CommandRuntime::default();
    let report = runtimes.participant_metrics(&team, 2, 540).await.unwrap();
    assert_eq!(report.participants.len(), 3);
    assert_eq!(report.pull_requests[0].number, 101);
    let asked: serde_json::Value =
        serde_json::from_str(&fake.read("last-participantMetrics.json")).unwrap();
    assert_eq!(
        asked,
        serde_json::json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "participantMetrics", "days": 2, "utcOffsetMinutes": 540 })
    );

    let runs = || fake.read("calls").matches("participantMetrics").count();
    runtimes.participant_metrics(&team, 2, 540).await.unwrap();
    assert_eq!(runs(), 1, "the same range and time zone are kept");
    runtimes.participant_metrics(&team, 2, 0).await.unwrap();
    assert_eq!(runs(), 2, "another time zone is another read");
    runtimes.participant_metrics(&team, 7, 0).await.unwrap();
    assert_eq!(runs(), 3, "and another range");

    // Where nothing is kept, each read runs the command.
    let unkept =
        CommandRuntime::default().keeping(std::time::Duration::ZERO, std::time::Duration::ZERO);
    let before = runs();
    unkept.participant_metrics(&team, 2, 540).await.unwrap();
    unkept.participant_metrics(&team, 2, 540).await.unwrap();
    assert_eq!(runs(), before + 2);

    fake.answer("participantMetrics", "not json", 1);
    let failing =
        CommandRuntime::default().keeping(std::time::Duration::ZERO, std::time::Duration::ZERO);
    let error = failing
        .participant_metrics(&team, 2, 540)
        .await
        .unwrap_err();
    assert_eq!(error.status, 503);
}

#[tokio::test]
async fn reads_activity_only_from_a_runtime_that_lists_it() {
    let since = agency_proto::Timestamp::parse("2026-11-24T11:00:00Z").unwrap();
    let fake = Fake::new();
    let error = CommandRuntime::default()
        .activity(&fake.team("app"), &since, 200)
        .await
        .unwrap_err();
    assert_eq!(error.status, 404);
    assert_eq!(
        fake.read("calls"),
        "describe\n",
        "no activity call to a runtime that does not list it"
    );

    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","activity"],"label":"App"}}"#,
        0,
    );
    fake.answer("activity", &format!(r#"{{"result":{ACTIVITY}}}"#), 0);
    let activity = CommandRuntime::default()
        .activity(&fake.team("app"), &since, 200)
        .await
        .unwrap();
    assert_eq!(activity.events.len(), 6);
    assert_eq!(fake.read("calls"), "describe\nactivity\n");
    let request: serde_json::Value =
        serde_json::from_str(&fake.read("last-activity.json")).unwrap();
    assert_eq!(request["op"], "activity");
    assert_eq!(
        request["since"], "2026-11-24T10:55:00Z",
        "a shared run starts five minutes before the first reader's since"
    );
    assert_eq!(
        request["limit"], 500,
        "and asks for the most, to be cut for each reader"
    );
}

#[tokio::test]
async fn times_each_decision_as_its_own_call_series_by_how_the_runtime_answered() {
    use agency_adapters::memory::ManualClock;
    use agency_adapters::metrics::Recorder;
    use agency_proto::metrics::SeriesKind;
    use std::sync::Arc;

    let fake = Fake::new();
    let recorder = Recorder::new(
        fake.path().join("metrics"),
        Arc::new(ManualClock::at("2026-11-24T12:00:00Z")),
    );
    let runtimes = CommandRuntime::default().recording(recorder.clone());
    let team = fake.team("app");
    runtimes.view(&team).await.unwrap();
    runtimes.decide(&team, decision("go")).await.unwrap();
    fake.answer(
        "decide",
        r#"{"error":{"status":404,"message":"no such request"}}"#,
        1,
    );
    runtimes.decide(&team, decision("go")).await.unwrap_err();
    fake.answer("decide", "Traceback: something broke", 1);
    runtimes.decide(&team, decision("go")).await.unwrap_err();

    let metrics = recorder.read(1, Some(SeriesKind::Call), None);
    let counted: Vec<(String, String, u64)> = metrics
        .series
        .iter()
        .map(|series| (series.name.clone(), series.status.clone(), series.count))
        .collect();
    assert_eq!(
        counted,
        vec![
            ("runtime decide".to_owned(), "error".to_owned(), 1),
            ("runtime decide".to_owned(), "ok".to_owned(), 2),
        ],
        "an answer of the runtime, a refusal too, is `ok`; a failure is `error`; reading the view is not a decision"
    );
}
