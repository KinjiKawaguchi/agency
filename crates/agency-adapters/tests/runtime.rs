//! The command runtime against a fake agency-runtime/1 runtime: a shell script that records each request
//! and answers with the response file prepared for that op.
use agency_adapters::runtime::CommandRuntime;
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::ports::TeamRuntime;
use agency_proto::{EnvelopeKind, Id, Payload, RuntimeDecision};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

const SAMPLE: &str = include_str!("../../../testdata/teamview.sample.json");
const USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");

const FAKE: &str = r#"#!/bin/sh
input=$(cat)
op=$(printf '%s' "$input" | sed -n 's/.*"op":"\([a-zA-Z]*\)".*/\1/p')
printf '%s' "$input" > "$FAKE_DIR/last-$op.json"
echo "$op" >> "$FAKE_DIR/calls"
cat "$FAKE_DIR/$op.json"
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
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let fake = Self { dir };
        fake.answer("describe", r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"App"}}"#, 0);
        fake.answer("view", &format!(r#"{{"result":{SAMPLE}}}"#), 0);
        fake.answer("decide", r#"{"result":{}}"#, 0);
        fake
    }

    fn answer(&self, op: &str, response: &str, exit: u8) {
        fs::write(self.dir.path().join(format!("{op}.json")), response).unwrap();
        fs::write(self.dir.path().join(format!("{op}.exit")), exit.to_string()).unwrap();
    }

    fn team(&self, team: &str) -> HostedTeam {
        let env = BTreeMap::from([("FAKE_DIR".to_owned(), self.dir.path().display().to_string())]);
        let runtime = RuntimeCommand::of(vec![self
            .dir
            .path()
            .join("runtime.sh")
            .display()
            .to_string()])
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

#[tokio::test]
async fn the_log_says_when_a_runtime_starts_failing_fails_differently_and_answers_again() {
    let fake = Fake::new();
    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let written = lines.clone();
    // Nothing is reused here, so each call runs the command and its outcome is seen.
    let runtimes = CommandRuntime::with_log(Box::new(move |line| {
        written.lock().unwrap().push(line.into())
    }))
    .keeping(std::time::Duration::ZERO, std::time::Duration::ZERO);
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

    fake.answer(
        "decide",
        r#"{"error":{"status":404,"message":"no such request"}}"#,
        1,
    );
    assert!(runtimes.decide(&team, decision("")).await.is_err());
    assert_eq!(
        logged(),
        ["agency: team acme/app: the team runtime answers again"],
        "a refusal (4xx) is an answer"
    );
    assert!(runtimes.decide(&team, decision("")).await.is_err());
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
    let runtime = CommandRuntime::default().keeping_activity(std::time::Duration::ZERO);
    let team = fake.team("app");
    for _ in 0..2 {
        runtime
            .activity(&team, &at("2026-11-24T11:00:30Z"), 200)
            .await
            .unwrap();
    }
    assert_eq!(fake.read("calls").matches("activity").count(), 2);
}
