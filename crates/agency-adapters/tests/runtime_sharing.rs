//! Readers of one team's runtime share its runs: one process for many readers, a fresh run after a decision, and
//! the last good view answered from memory while a slow or failing runtime refreshes.
mod common;

use agency_adapters::runtime::CommandRuntime;
use agency_core::ports::TeamRuntime;
use common::*;

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
