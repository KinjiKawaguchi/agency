//! The CLI's choices that decide what a node does: `init --no-local-clients`, the state directory a team on this
//! node needs, and `join`'s confirmation. Each is run through the `agency` binary, as the owner types it.
mod common;

use agency_adapters::files::DataDir;
use agency_core::config::NodeConfig;
use agency_core::invites;
use agency_core::ports::ConfigStore;
use agency_proto::Role;
use common::{alice, id};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

/// Runs `agency --dir <dir> <args>`, with `stdin` as what is typed (none when it is `None`).
fn agency(dir: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_agency"))
        .arg("--dir")
        .arg(dir)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(typed) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(typed.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

/// `agency` run off the async runtime, so that a node the test serves keeps answering it.
async fn agency_async(dir: PathBuf, args: Vec<String>, stdin: Option<String>) -> Output {
    tokio::task::spawn_blocking(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        agency(&dir, &args, stdin.as_deref())
    })
    .await
    .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `agency init` for account `account` in `dir`, with `extra` arguments.
fn init(dir: &Path, account: &str, extra: &[&str]) -> Output {
    let mut args = vec!["init", "--account", account, "--label", account];
    args.extend(["--url", "http://127.0.0.1:1", "--listen", "127.0.0.1:1"]);
    args.extend(extra);
    agency(dir, &args, None)
}

fn config_of(dir: &Path) -> NodeConfig {
    DataDir::new(dir.to_owned()).load_config().unwrap()
}

#[test]
fn init_writes_whether_the_node_admits_local_clients_as_it_is_told() {
    let root = TempDir::new().unwrap();
    const HINT: &str = "Every process on this machine that reaches 127.0.0.1 is admitted";

    let shared = root.path().join("shared");
    let output = init(&shared, "alice", &["--no-local-clients"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(
        !config_of(&shared).local_clients,
        "--no-local-clients refuses local clients"
    );
    assert!(!text(&output.stdout).contains(HINT), "and says no hint");

    let own = root.path().join("own");
    let output = init(&own, "alice", &[]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(
        config_of(&own).local_clients,
        "without it, local clients are admitted"
    );
    assert!(
        text(&output.stdout).contains(HINT),
        "and the hint says so: {}",
        text(&output.stdout)
    );
}

#[test]
fn a_team_on_this_node_needs_a_state_directory_and_nothing_changes_without_one() {
    let root = TempDir::new().unwrap();
    let dir = root.path().join("node");
    assert!(init(&dir, "alice", &[]).status.success());
    let created = agency(
        &dir,
        &["org-create", "--org", "acme", "--label", "Acme"],
        None,
    );
    assert!(created.status.success(), "{}", text(&created.stderr));

    let args = [
        "team-add", "--org", "acme", "--team", "app", "--label", "App",
    ];
    let refused = agency(&dir, &args, None);
    assert!(!refused.status.success());
    assert!(
        text(&refused.stderr).contains("a team on this node needs --state-dir"),
        "{}",
        text(&refused.stderr)
    );
    assert!(config_of(&dir).teams.is_empty(), "no team in config.json");
    let status = text(&agency(&dir, &["status"], None).stdout);
    assert!(
        status.contains("Org acme (Acme), version 1,") && status.contains("teams: \n"),
        "the org has no new version and no team: {status}"
    );

    let state = root.path().join("state");
    let added = agency(
        &dir,
        &[&args[..], &["--state-dir", state.to_str().unwrap()]].concat(),
        None,
    );
    assert!(added.status.success(), "{}", text(&added.stderr));
    assert_eq!(config_of(&dir).teams.len(), 1, "with one, it is added");
}

#[tokio::test]
async fn join_asks_before_it_joins_unless_told_yes() {
    let (alice, _) = alice().await;
    // The CLI signs its join with the machine's time, which alice's node checks against its own.
    let now = agency_proto::clock::OffsetDateTime::now_utc();
    alice.clock.set(&agency_proto::clock::format(now));
    let link = || invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let root = TempDir::new().unwrap();
    let is_member = |dir: &Path| {
        let status = text(&agency(dir, &["status"], None).stdout);
        status.contains("Org acme (Acme)") && status.contains(", member,")
    };

    for (name, answer) in [("bob", "n\n"), ("carol", "\n")] {
        let dir = root.path().join(name);
        assert!(init(&dir, name, &[]).status.success());
        let args = vec!["join".to_owned(), link()];
        let output = agency_async(dir.clone(), args, Some(answer.into())).await;
        assert!(!output.status.success(), "{answer:?} is no");
        assert!(
            text(&output.stdout).contains("Join the org \"Acme\" (acme)?"),
            "it asked: {}",
            text(&output.stdout)
        );
        assert!(
            text(&output.stderr).contains("joining was cancelled"),
            "{}",
            text(&output.stderr)
        );
        assert!(!is_member(&dir), "{name} did not join");
    }

    let dave = root.path().join("dave");
    assert!(init(&dave, "dave", &[]).status.success());
    let output = agency_async(
        dave.clone(),
        vec!["join".into(), link()],
        Some("y\n".into()),
    )
    .await;
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(is_member(&dave), "y joins");

    let erin = root.path().join("erin");
    assert!(init(&erin, "erin", &[]).status.success());
    let args = vec!["join".to_owned(), link(), "--yes".to_owned()];
    let output = agency_async(erin.clone(), args, None).await;
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(
        !text(&output.stdout).contains("Join the org"),
        "--yes asks nothing: {}",
        text(&output.stdout)
    );
    assert!(is_member(&erin), "--yes joins without an answer");
}
