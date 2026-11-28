//! The release a team's runtime says it is (`describe`), kept in `runtime-releases.json` of the data directory.
mod common;

use agency_adapters::files::DataDir;
use agency_adapters::memory::ManualClock;
use agency_adapters::runtime::CommandRuntime;
use agency_adapters::runtime_releases::{read, ReleaseLog};
use agency_core::ports::TeamRuntime;
use agency_proto::Id;
use common::*;
use tempfile::TempDir;

/// A runtime that keeps its releases in the data directory `data` for the teams `hosted` (acme/<name>), whose
/// clock stands at 2026-11-28T01:00:00Z.
fn reporting_for(data: &TempDir, hosted: &[&str]) -> CommandRuntime {
    let clock = ManualClock::at("2026-11-28T01:00:00Z");
    let acme = id("acme");
    let teams: Vec<Id> = hosted.iter().map(|name| id(name)).collect();
    let log = ReleaseLog::new(
        &DataDir::new(data.path().to_owned()),
        std::sync::Arc::new(clock),
        teams.iter().map(|team| (&acme, team)),
    );
    CommandRuntime::default().reporting_releases(log)
}

fn reporting(data: &TempDir) -> CommandRuntime {
    reporting_for(data, &["app", "old", "web"])
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

#[tokio::test]
async fn two_processes_add_to_the_file_and_do_not_write_back_an_old_copy() {
    let data = TempDir::new().unwrap();
    let fake = Fake::new();
    // `serve` and a command both started before either read `describe`.
    let serve = reporting(&data);
    let command = reporting(&data);
    fake.answer("describe", &describe_with(r#","version":"0.7.2""#), 0);

    serve.view(&fake.team("app")).await.unwrap();
    command
        .decide(&fake.team("web"), decision("x"))
        .await
        .unwrap();

    let kept = read(&DataDir::new(data.path().to_owned()));
    assert_eq!(
        kept["acme/app"].version.as_deref(),
        Some("0.7.2"),
        "what the other process wrote is still there"
    );
    assert!(kept.contains_key("acme/web"));
}

#[tokio::test]
async fn the_entry_of_a_team_the_node_no_longer_hosts_is_dropped() {
    let data = TempDir::new().unwrap();
    let fake = Fake::new();
    reporting_for(&data, &["app", "old"])
        .decide(&fake.team("old"), decision("x"))
        .await
        .unwrap();
    assert!(read(&DataDir::new(data.path().to_owned())).contains_key("acme/old"));

    reporting_for(&data, &["app"])
        .view(&fake.team("app"))
        .await
        .unwrap();

    let kept = read(&DataDir::new(data.path().to_owned()));
    assert!(kept.contains_key("acme/app"));
    assert!(!kept.contains_key("acme/old"), "{kept:?}");
}
