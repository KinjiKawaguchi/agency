//! A node that finds data or settings of a newer release says so and stops, and writes nothing into the
//! directory ("Node data and configuration" in docs/compatibility.md).
use agency_adapters::files::DataDir;
use agency_core::config::{Account, NodeConfig};
use agency_core::ports::ConfigStore;
use agency_node::{open, Overrides};
use agency_proto::Id;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// A data directory as an earlier release left it: an account key and a configuration, and no `format` file.
fn data_dir_of(root: &Path) -> DataDir {
    let data = DataDir::new(root.join("node"));
    data.create_key().unwrap();
    let account = Account {
        id: Id::parse("alice").unwrap(),
        label: "Alice".into(),
    };
    data.save_config(&NodeConfig::new(
        "http://127.0.0.1:1",
        "127.0.0.1:1",
        account,
        "scrum",
    ))
    .unwrap();
    data
}

/// Every file under `dir` with its bytes.
fn contents(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(dir).unwrap().display().to_string();
                files.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn refusal(data: DataDir) -> String {
    match open(data, Overrides::default()) {
        Ok(_) => panic!("the node started"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn data_of_a_newer_format_stops_the_node_and_nothing_is_written() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    std::fs::write(data.root().join("format"), "2\n").unwrap();
    let before = contents(data.root());

    let message = refusal(data.clone());

    assert!(message.contains("newer release"), "{message}");
    assert!(message.contains("data format 2"), "{message}");
    assert!(
        !message.contains(&root.path().display().to_string()),
        "the message names no path: {message}"
    );
    assert_eq!(contents(data.root()), before);
}

#[test]
fn the_format_is_read_before_a_key_file_a_newer_release_may_have_changed() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    std::fs::write(data.root().join("format"), "2\n").unwrap();
    std::fs::write(data.root().join("account.key"), "a key in a new encoding\n").unwrap();

    let message = refusal(data);

    assert!(message.contains("newer release"), "{message}");
}

#[test]
fn a_format_file_that_holds_no_number_stops_the_node_and_nothing_is_written() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    std::fs::write(data.root().join("format"), "two\n").unwrap();
    let before = contents(data.root());

    let message = refusal(data.clone());

    assert!(message.contains("does not hold a number"), "{message}");
    assert_eq!(contents(data.root()), before);
}

#[test]
fn a_format_file_that_cannot_be_read_stops_the_node_and_says_so() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    std::fs::create_dir(data.root().join("format")).unwrap();
    let before = contents(data.root());

    let message = refusal(data.clone());

    assert!(message.contains("cannot be read"), "{message}");
    assert!(!message.contains("does not hold a number"), "{message}");
    assert_eq!(contents(data.root()), before);
}

#[test]
fn a_directory_without_a_format_file_starts_and_gets_format_1() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    assert!(!data.root().join("format").exists());

    open(data.clone(), Overrides::default()).unwrap();

    assert_eq!(
        std::fs::read_to_string(data.root().join("format")).unwrap(),
        "1\n"
    );
    open(data.clone(), Overrides::default()).unwrap();
    assert_eq!(
        std::fs::read_to_string(data.root().join("format")).unwrap(),
        "1\n"
    );
}

#[test]
fn a_setting_this_release_does_not_know_says_it_may_come_from_a_newer_release() {
    for edit in [
        |config: &mut serde_json::Value| config["freshSetting"] = true.into(),
        |config: &mut serde_json::Value| config["logging"] = serde_json::json!({"fresh": 1}),
    ] {
        let root = TempDir::new().unwrap();
        let data = data_dir_of(root.path());
        let mut config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(data.config_path()).unwrap()).unwrap();
        edit(&mut config);
        std::fs::write(data.config_path(), config.to_string()).unwrap();
        let before = contents(data.root());

        let message = refusal(data.clone());

        assert!(message.contains("newer release"), "{message}");
        assert!(message.contains("unknown field"), "{message}");
        assert!(
            !message.contains("agency init"),
            "not the advice for a missing file: {message}"
        );
        assert_eq!(contents(data.root()), before, "no format file is written");
    }
}

#[test]
fn a_configuration_that_cannot_be_read_for_another_reason_keeps_its_own_message() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    std::fs::write(data.config_path(), "{").unwrap();

    let message = refusal(data);

    assert!(message.contains("agency init"), "{message}");
    assert!(!message.contains("newer release"), "{message}");
}

#[test]
fn every_command_that_reads_the_directory_refuses_a_newer_format() {
    let root = TempDir::new().unwrap();
    let data = data_dir_of(root.path());
    std::fs::write(data.root().join("format"), "2\n").unwrap();
    for command in [["logs"].as_slice(), &["metrics"], &["funding", "list"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_agency"))
            .arg("--dir")
            .arg(data.root())
            .args(command)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{command:?}");
        assert!(stderr.contains("newer release"), "{command:?}: {stderr}");
    }
}

#[test]
fn init_writes_format_1_and_refuses_a_directory_of_a_newer_format() {
    let root = TempDir::new().unwrap();
    let dir = root.path().join("fresh");
    let init = |dir: &Path| {
        Command::new(env!("CARGO_BIN_EXE_agency"))
            .arg("--dir")
            .arg(dir)
            .args(["init", "--account", "alice", "--label", "Alice"])
            .args(["--url", "http://127.0.0.1:1", "--listen", "127.0.0.1:1"])
            .output()
            .unwrap()
    };

    assert!(init(&dir).status.success());
    assert_eq!(std::fs::read_to_string(dir.join("format")).unwrap(), "1\n");

    let newer = root.path().join("newer");
    std::fs::create_dir(&newer).unwrap();
    std::fs::write(newer.join("format"), "2\n").unwrap();
    let output = init(&newer);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("newer release"));
    assert!(!newer.join("account.key").exists());
}
