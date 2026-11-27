//! The command runtime against a fake agency-runtime/1 runtime: a shell script that records each request
//! and answers with the response file prepared for that op.
use agency_adapters::files::DataDir;
use agency_adapters::memory::ManualClock;
use agency_adapters::runtime::CommandRuntime;
use agency_adapters::runtime_releases::{read, ReleaseLog};
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::ports::TeamRuntime;
use agency_proto::{EnvelopeKind, Id, Payload, RuntimeDecision};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const SAMPLE: &str = include_str!("../../../testdata/teamview.sample.json");
const USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");
const PARTICIPANTS: &str = include_str!("../../../testdata/participantmetrics.sample.json");

/// The fake runtime's script. It is run as `sh runtime.sh`, never executed itself: a program that was just
/// written cannot be executed while any process still holds it open for writing (`ETXTBSY`, "Text file
/// busy"), and another test's `fork` can hold that descriptor for a moment, which failed a test now and then.
/// A file that only `sh` reads has no such wait, and the shell it starts is never written by a test.
const FAKE: &str = r#"input=$(cat)
op=$(printf '%s' "$input" | sed -n 's/.*"op":"\([a-zA-Z]*\)".*/\1/p')
printf '%s' "$input" > "$FAKE_DIR/last-$op.json"
echo "$op" >> "$FAKE_DIR/calls"
answer=$(cat "$FAKE_DIR/$op.json")
sleep "$(cat "$FAKE_DIR/$op.delay" 2>/dev/null || echo 0)"
printf '%s' "$answer"
exit "$(cat "$FAKE_DIR/$op.exit" 2>/dev/null || echo 0)"
"#;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

struct Fake {
    dir: TempDir,
}

impl Fake {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("runtime.sh");
        fs::write(&script, FAKE).unwrap();
        let fake = Self { dir };
        fake.answer("describe", r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"App"}}"#, 0);
        fake.answer("view", &format!(r#"{{"result":{SAMPLE}}}"#), 0);
        fake.answer("decide", r#"{"result":{}}"#, 0);
        fake
    }

    /// Makes the runtime take `seconds` (a number, maybe fractional) to answer `op`.
    fn delay(&self, op: &str, seconds: &str) {
        fs::write(self.dir.path().join(format!("{op}.delay")), seconds).unwrap();
    }

    fn answer(&self, op: &str, response: &str, exit: u8) {
        fs::write(self.dir.path().join(format!("{op}.json")), response).unwrap();
        fs::write(self.dir.path().join(format!("{op}.exit")), exit.to_string()).unwrap();
    }

    fn team(&self, team: &str) -> HostedTeam {
        let env = BTreeMap::from([("FAKE_DIR".to_owned(), self.dir.path().display().to_string())]);
        let script = self.dir.path().join("runtime.sh");
        let runtime = RuntimeCommand::of(vec!["/bin/sh".to_owned(), script.display().to_string()])
            .with_env(env);
        HostedTeam::new(id("acme"), id(team), id("alice"), runtime)
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join(name)).unwrap_or_default()
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

fn decision(text: &str) -> RuntimeDecision {
    RuntimeDecision {
        kind: EnvelopeKind::Answer,
        target: "r-20261124-110000-a1b2".into(),
        by: id("bob"),
        by_participant: None,
        on_behalf_of: None,
        payload: Payload {
            text: text.into(),
            choice: Some("a".into()),
        },
    }
}

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
async fn readers_share_one_run_of_the_command_and_a_decision_reruns_it() {
    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","tokenUsage"],"label":"App"}}"#,
        0,
    );
    fake.answer("tokenUsage", &format!(r#"{{"result":{USAGE}}}"#), 0);
    let runtimes = std::sync::Arc::new(CommandRuntime::default());
    let team = fake.team("app");
    let readers = (0..20).map(|_| {
        let (runtimes, team) = (runtimes.clone(), team.clone());
        tokio::spawn(async move { runtimes.view(&team).await.map(|view| view.requests.len()) })
    });
    for reader in readers {
        assert_eq!(reader.await.unwrap().unwrap(), 3);
    }
    assert_eq!(
        fake.read("calls"),
        "describe\nview\n",
        "20 readers, one run"
    );

    runtimes.decide(&team, decision("")).await.unwrap();
    runtimes.view(&team).await.unwrap();
    assert_eq!(
        fake.read("calls"),
        "describe\nview\ndecide\nview\n",
        "a decision changes the view, so the next read runs the command"
    );

    for _ in 0..3 {
        runtimes.token_usage(&team, 7).await.unwrap();
    }
    runtimes.token_usage(&team, 14).await.unwrap();
    assert_eq!(
        fake.read("calls").matches("tokenUsage").count(),
        2,
        "usage is kept per range"
    );
}

/// A runtime that fails because a command is missing says so once, with what to do, and a failure of
/// another kind does not.
#[tokio::test]
async fn the_log_says_what_to_do_when_a_command_of_the_runtime_is_not_found() {
    let fake = Fake::new();
    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let written = lines.clone();
    let runtimes = CommandRuntime::with_log(Box::new(move |line| {
        written.lock().unwrap().push(line.into())
    }))
    .keeping(std::time::Duration::ZERO, std::time::Duration::ZERO)
    .serving_stale_for(std::time::Duration::ZERO);
    let team = fake.team("app");
    let logged = || std::mem::take(&mut *lines.lock().unwrap());
    runtimes.view(&team).await.unwrap();
    logged();

    // The runtime runs, and the command it runs is not on its PATH (what the scrum harness says).
    fake.answer(
        "view",
        r#"{"error":{"status":503,"message":"spawnSync gh ENOENT"}}"#,
        0,
    );
    for _ in 0..3 {
        assert!(runtimes.view(&team).await.is_err());
    }
    let missing = logged();
    assert_eq!(
        missing.len(),
        2,
        "the failure and what to do, once and not on every call: {missing:?}"
    );
    assert_eq!(
        missing[0],
        "agency: team acme/app: the team runtime fails: spawnSync gh ENOENT"
    );
    assert!(
        missing[1].starts_with("agency: team acme/app: a command the team runtime runs was not found: put its directory on the PATH of the launchd agent or the systemd unit"),
        "{missing:?}"
    );

    // Another failure is logged as before, with no advice about the PATH.
    fake.answer(
        "view",
        r#"{"error":{"status":503,"message":"the board is locked"}}"#,
        0,
    );
    assert!(runtimes.view(&team).await.is_err());
    assert_eq!(
        logged(),
        ["agency: team acme/app: the team runtime fails: the board is locked"]
    );

    // The runtime's own command is not there.
    let gone = HostedTeam::new(
        id("acme"),
        id("gone"),
        id("alice"),
        RuntimeCommand::of(vec!["/nonexistent/runtime".into()]),
    );
    assert!(runtimes.view(&gone).await.is_err());
    let own = logged();
    assert_eq!(own.len(), 2, "{own:?}");
    assert!(
        own[0].contains("cannot start the team runtime: No such file or directory"),
        "{own:?}"
    );
    assert!(
        own[1].contains("a command the team runtime runs was not found"),
        "{own:?}"
    );
}

#[tokio::test]
async fn the_log_says_when_a_runtime_starts_failing_fails_differently_and_answers_again() {
    let fake = Fake::new();
    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let written = lines.clone();
    // Nothing is reused here, so each call runs the command and its outcome is seen.
    let runtimes = CommandRuntime::with_log(Box::new(move |line| {
        written.lock().unwrap().push(line.into())
    }))
    .keeping(std::time::Duration::ZERO, std::time::Duration::ZERO)
    .serving_stale_for(std::time::Duration::ZERO);
    let team = fake.team("app");
    let logged = || std::mem::take(&mut *lines.lock().unwrap());

    runtimes.view(&team).await.unwrap();
    assert_eq!(
        logged(),
        Vec::<String>::new(),
        "a runtime that answers is not logged"
    );

    fake.answer("view", "not json", 1);
    for _ in 0..3 {
        assert!(runtimes.view(&team).await.is_err());
    }
    let failing = logged();
    assert_eq!(
        failing.len(),
        1,
        "logged once, not on every call: {failing:?}"
    );
    assert!(
        failing[0].starts_with("agency: team acme/app: the team runtime fails: the team runtime did not write an agency-runtime/1 response"),
        "{failing:?}"
    );

    fake.answer(
        "view",
        r#"{"error":{"status":503,"message":"the board is locked"}}"#,
        1,
    );
    assert!(runtimes.view(&team).await.is_err());
    assert_eq!(
        logged(),
        ["agency: team acme/app: the team runtime fails: the board is locked"],
        "a different failure is logged"
    );

    // A decision starts a refresh of the view in the background, so the view answers here: the refresh
    // is not what this part is about, and it is given time to finish before the next step.
    fake.answer("view", &format!(r#"{{"result":{SAMPLE}}}"#), 0);
    fake.answer(
        "decide",
        r#"{"error":{"status":404,"message":"no such request"}}"#,
        1,
    );
    assert!(runtimes.decide(&team, decision("")).await.is_err());
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        logged(),
        ["agency: team acme/app: the team runtime answers again"],
        "a refusal (4xx) is an answer"
    );
    assert!(runtimes.decide(&team, decision("")).await.is_err());
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(logged(), Vec::<String>::new(), "and is never logged itself");

    fake.answer("view", "not json", 1);
    assert!(runtimes.view(&team).await.is_err());
    fake.answer("view", &format!(r#"{{"result":{SAMPLE}}}"#), 0);
    runtimes.view(&team).await.unwrap();
    runtimes.view(&team).await.unwrap();
    let recovered = logged();
    assert_eq!(recovered.len(), 2, "{recovered:?}");
    assert_eq!(
        recovered[1],
        "agency: team acme/app: the team runtime answers again"
    );

    let other = fake.team("web");
    fake.answer("view", "not json", 1);
    assert!(runtimes.view(&other).await.is_err());
    assert!(runtimes.view(&team).await.is_err());
    assert_eq!(logged().len(), 2, "each team has its own state");
}

const ACTIVITY: &str = include_str!("../../../testdata/activity.sample.json");

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

fn activity_fake(truncated: bool) -> Fake {
    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","activity"],"label":"App"}}"#,
        0,
    );
    let answer = ACTIVITY.replace(
        "\"truncated\": false",
        &format!("\"truncated\": {truncated}"),
    );
    fake.answer("activity", &format!(r#"{{"result":{answer}}}"#), 0);
    fake
}

fn at(raw: &str) -> agency_proto::Timestamp {
    agency_proto::Timestamp::parse(raw).unwrap()
}

#[tokio::test]
async fn twenty_readers_of_a_teams_activity_start_one_process() {
    let fake = activity_fake(false);
    let runtime = std::sync::Arc::new(CommandRuntime::default());
    let team = fake.team("app");
    let readers = (0..20).map(|n| {
        let (runtime, team) = (runtime.clone(), team.clone());
        // Each reader asks from its own time, as the default (an hour before the read) differs by a second.
        let since = at(&format!("2026-11-24T11:00:{:02}Z", 30 + n));
        tokio::spawn(async move { runtime.activity(&team, &since, 200).await })
    });
    for reader in readers {
        assert_eq!(reader.await.unwrap().unwrap().events.len(), 6);
    }
    assert_eq!(
        fake.read("calls"),
        "describe\nactivity\n",
        "20 readers with different `since` values, one run"
    );
    let later = runtime
        .activity(&team, &at("2026-11-24T11:26:00Z"), 200)
        .await
        .unwrap();
    assert_eq!(
        later.events.len(),
        3,
        "cut to this reader's since, from the same run"
    );
    let cut = runtime
        .activity(&team, &at("2026-11-24T11:00:30Z"), 2)
        .await
        .unwrap();
    assert_eq!((cut.events.len(), cut.truncated), (2, true));
    assert_eq!(fake.read("calls").matches("activity").count(), 1);
}

#[tokio::test]
async fn a_reader_the_shared_run_cannot_answer_gets_its_own_run() {
    let fake = activity_fake(false);
    let runtime = CommandRuntime::default();
    let team = fake.team("app");
    runtime
        .activity(&team, &at("2026-11-24T11:30:00Z"), 200)
        .await
        .unwrap();
    let older = runtime
        .activity(&team, &at("2026-11-24T10:00:00Z"), 200)
        .await
        .unwrap();
    assert_eq!(
        older.events.len(),
        6,
        "asked from before the shared run began"
    );
    assert_eq!(fake.read("calls").matches("activity").count(), 2);
    let request: serde_json::Value =
        serde_json::from_str(&fake.read("last-activity.json")).unwrap();
    assert_eq!(request["since"], "2026-11-24T10:00:00Z");
}

#[tokio::test]
async fn a_run_cut_at_the_runtimes_limit_answers_only_what_it_surely_holds() {
    let fake = activity_fake(true);
    let runtime = CommandRuntime::default();
    let team = fake.team("app");
    let small = runtime
        .activity(&team, &at("2026-11-24T11:00:00Z"), 3)
        .await
        .unwrap();
    assert_eq!((small.events.len(), small.truncated), (3, true));
    assert_eq!(
        fake.read("calls").matches("activity").count(),
        1,
        "three events are surely there"
    );
    runtime
        .activity(&team, &at("2026-11-24T11:00:00Z"), 200)
        .await
        .unwrap();
    assert_eq!(
        fake.read("calls").matches("activity").count(),
        2,
        "a reader who wants more than the cut run holds asks the runtime"
    );
}

#[tokio::test]
async fn a_shared_run_is_not_reused_once_its_time_is_up() {
    let fake = activity_fake(false);
    let runtime = CommandRuntime::default()
        .keeping_activity(std::time::Duration::ZERO)
        .serving_stale_for(std::time::Duration::ZERO);
    let team = fake.team("app");
    for _ in 0..2 {
        runtime
            .activity(&team, &at("2026-11-24T11:00:30Z"), 200)
            .await
            .unwrap();
    }
    assert_eq!(fake.read("calls").matches("activity").count(), 2);
}

use std::time::{Duration, Instant};

/// A view with a different label, to tell a refreshed answer from the one it replaced.
fn view_labeled(label: &str) -> String {
    let mut view: serde_json::Value = serde_json::from_str(SAMPLE).unwrap();
    view["label"] = serde_json::json!(label);
    format!(r#"{{"result":{view}}}"#)
}

/// Polls until `done`, for at most `seconds`.
async fn until(seconds: u64, mut done: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(seconds);
    while !done() {
        assert!(Instant::now() < end, "it did not happen in time");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_read_answers_from_memory_while_a_slow_runtime_refreshes() {
    let fake = Fake::new();
    fake.answer("view", &view_labeled("before"), 0);
    let runtime =
        CommandRuntime::default().keeping(Duration::from_millis(50), Duration::from_secs(60));
    let team = fake.team("app");
    assert_eq!(runtime.view(&team).await.unwrap().label, "before");

    // The answer in memory is now old, and the runtime has become slow and changed its answer.
    tokio::time::sleep(Duration::from_millis(100)).await;
    fake.answer("view", &view_labeled("after"), 0);
    fake.delay("view", "1.5");
    let started = Instant::now();
    let old = runtime.view(&team).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "a read waits for no runtime process (the owner's bound is p95 100 ms): {:?}",
        started.elapsed()
    );
    assert_eq!(
        old.label, "before",
        "the answer in memory, while it refreshes"
    );

    until(5, || fake.read("calls").matches("view").count() == 2).await;
    tokio::time::sleep(Duration::from_millis(1800)).await;
    let started = Instant::now();
    assert_eq!(runtime.view(&team).await.unwrap().label, "after");
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "the refreshed answer is in memory"
    );
    let reads = runtime.view_reads();
    assert_eq!(
        (reads.cold, reads.stale >= 1),
        (1, true),
        "only the first read waited: {reads:?}"
    );
}

#[tokio::test]
async fn after_a_decision_readers_share_the_refresh_that_starts_at_once() {
    let fake = Fake::new();
    let runtime = std::sync::Arc::new(CommandRuntime::default());
    let team = fake.team("app");
    runtime.view(&team).await.unwrap();
    runtime.decide(&team, decision("")).await.unwrap();
    let readers = (0..10).map(|_| {
        let (runtime, team) = (runtime.clone(), team.clone());
        tokio::spawn(async move { runtime.view(&team).await.map(|view| view.requests.len()) })
    });
    for reader in readers {
        assert_eq!(reader.await.unwrap().unwrap(), 3);
    }
    assert_eq!(
        fake.read("calls"),
        "describe\nview\ndecide\nview\n",
        "ten readers after a decision find the one refresh that began with it"
    );
}

#[tokio::test]
async fn a_failing_refresh_keeps_the_last_good_view_until_it_is_too_old() {
    let fake = Fake::new();
    let runtime = CommandRuntime::default()
        .keeping(Duration::from_millis(50), Duration::from_secs(60))
        .serving_stale_for(Duration::from_millis(600));
    let team = fake.team("app");
    runtime.view(&team).await.unwrap();
    fake.answer("view", "not json", 1);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        runtime.view(&team).await.is_ok(),
        "old, so served, and refreshed in the background"
    );
    until(5, || fake.read("calls").matches("view").count() == 2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        runtime.view(&team).await.is_ok(),
        "the refresh failed; the last good answer is kept"
    );
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(
        runtime.view(&team).await.is_err(),
        "too old to serve: the read runs the command and fails"
    );
}

#[tokio::test]
async fn a_run_that_began_before_a_decision_is_not_kept() {
    let fake = Fake::new();
    fake.answer("view", &view_labeled("pre"), 0);
    let runtime =
        CommandRuntime::default().keeping(Duration::from_millis(50), Duration::from_secs(60));
    let team = fake.team("app");
    assert_eq!(runtime.view(&team).await.unwrap().label, "pre");

    // A refresh begins and is slow; its answer, taken when it began, is the one before the decision.
    tokio::time::sleep(Duration::from_millis(100)).await;
    fake.delay("view", "1");
    assert_eq!(runtime.view(&team).await.unwrap().label, "pre");
    tokio::time::sleep(Duration::from_millis(100)).await;
    fake.answer("view", &view_labeled("post"), 0);
    runtime.decide(&team, decision("")).await.unwrap();

    // The refresh that began before the decision ends (about 1 s after it began) and must not be kept: the
    // read after it waits for the run that began with the decision, and finds its answer.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(runtime.view(&team).await.unwrap().label, "post");
}

#[tokio::test]
async fn readers_of_a_runtime_that_stays_down_share_one_failure() {
    let fake = Fake::new();
    let runtime = std::sync::Arc::new(
        CommandRuntime::default()
            .keeping(Duration::from_millis(150), Duration::from_secs(60))
            .serving_stale_for(Duration::from_millis(300)),
    );
    let team = fake.team("app");
    runtime.view(&team).await.unwrap();

    // The good answer is now too old to serve, and the runtime fails, slowly.
    tokio::time::sleep(Duration::from_millis(400)).await;
    fake.answer("view", "not json", 1);
    fake.delay("view", "0.6");
    let readers = (0..10).map(|_| {
        let (runtime, team) = (runtime.clone(), team.clone());
        tokio::spawn(async move { runtime.view(&team).await.is_err() })
    });
    for reader in readers {
        assert!(reader.await.unwrap(), "every reader gets the failure");
    }
    assert_eq!(
        fake.read("calls").matches("view").count(),
        2,
        "ten readers, one run of the failing command"
    );

    assert!(runtime.view(&team).await.is_err());
    assert_eq!(
        fake.read("calls").matches("view").count(),
        2,
        "a read soon after finds the failure kept and starts no run"
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(runtime.view(&team).await.is_err());
    assert_eq!(
        fake.read("calls").matches("view").count(),
        3,
        "once it is old, the command is run again"
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

/// A runtime that keeps its releases in the data directory `data`, whose clock stands at 2026-11-28T01:00:00Z.
fn reporting(data: &TempDir) -> CommandRuntime {
    let clock = ManualClock::at("2026-11-28T01:00:00Z");
    let log = ReleaseLog::new(
        &DataDir::new(data.path().to_owned()),
        std::sync::Arc::new(clock),
    );
    CommandRuntime::default().reporting_releases(log)
}

fn describe_with(version: &str) -> String {
    format!(
        r#"{{"result":{{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"App"{version}}}}}"#
    )
}

#[tokio::test]
async fn notes_the_release_a_runtime_says_and_that_one_that_does_not_say_has_none() {
    let data = TempDir::new().unwrap();
    let runtimes = reporting(&data);
    let said = Fake::new();
    said.answer("describe", &describe_with(r#","version":"0.7.2""#), 0);
    let silent = Fake::new();

    runtimes.view(&said.team("app")).await.unwrap();
    runtimes
        .decide(&silent.team("old"), decision("x"))
        .await
        .unwrap();

    let kept = read(&DataDir::new(data.path().to_owned()));
    let app = &kept["acme/app"];
    assert_eq!(app.version.as_deref(), Some("0.7.2"));
    assert_eq!(app.read_at, "2026-11-28T01:00:00Z");
    assert_eq!(kept["acme/old"].version, None, "read, and said nothing");
    let file = std::fs::read_to_string(data.path().join("runtime-releases.json")).unwrap();
    assert!(
        !file.contains("null"),
        "a missing release is left out: {file}"
    );
}

#[tokio::test]
async fn a_release_beyond_the_limits_is_not_kept_and_the_runtime_is_still_served() {
    let data = TempDir::new().unwrap();
    let runtimes = reporting(&data);
    let fake = Fake::new();
    let long = "v".repeat(65);
    fake.answer(
        "describe",
        &describe_with(&format!(r#","version":"{long}""#)),
        0,
    );

    assert!(runtimes.view(&fake.team("app")).await.is_ok());

    let kept = read(&DataDir::new(data.path().to_owned()));
    assert_eq!(kept["acme/app"].version, None);
}

#[tokio::test]
async fn describe_is_read_once_so_the_file_is_written_once_per_team_and_keeps_the_others() {
    let data = TempDir::new().unwrap();
    let fake = Fake::new();
    fake.answer("describe", &describe_with(r#","version":"0.7.2""#), 0);
    reporting(&data).view(&fake.team("app")).await.unwrap();

    // A node started later notes another team and keeps what the file already held.
    fake.answer("describe", &describe_with(r#","version":"0.8.0""#), 0);
    let later = reporting(&data);
    later
        .decide(&fake.team("web"), decision("x"))
        .await
        .unwrap();
    later
        .decide(&fake.team("web"), decision("y"))
        .await
        .unwrap();

    let kept = read(&DataDir::new(data.path().to_owned()));
    assert_eq!(kept["acme/app"].version.as_deref(), Some("0.7.2"));
    assert_eq!(kept["acme/web"].version.as_deref(), Some("0.8.0"));
}

#[tokio::test]
async fn a_runtime_that_does_not_speak_agency_runtime_1_is_noted_and_refused() {
    let data = TempDir::new().unwrap();
    let runtimes = reporting(&data);
    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/2"],"ops":["describe"],"label":"App","version":"9.0.0"}}"#,
        0,
    );

    assert!(runtimes.view(&fake.team("app")).await.is_err());

    let kept = read(&DataDir::new(data.path().to_owned()));
    assert_eq!(kept["acme/app"].version.as_deref(), Some("9.0.0"));
}
