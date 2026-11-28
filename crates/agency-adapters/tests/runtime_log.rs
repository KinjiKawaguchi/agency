//! What the node's log says of a team's runtime: a command that is not found, and a runtime that starts
//! failing, fails another way, or answers again.
mod common;

use agency_adapters::runtime::CommandRuntime;
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::ports::TeamRuntime;
use common::*;

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
