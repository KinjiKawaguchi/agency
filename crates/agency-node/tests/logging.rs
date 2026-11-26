//! The node's log file: what reaches it, where, and what stops a node from starting.
mod common;

use agency_adapters::files::DataDir;
use agency_core::config::{AccessConfig, Account, NodeConfig};
use agency_core::ports::ConfigStore as _;
use agency_node::Overrides;
use common::{id, memory_runtime, node, T0};

#[tokio::test]
async fn a_line_the_node_would_have_printed_goes_to_the_log_file_with_the_nodes_time() {
    let (runtime, _) = memory_runtime();
    let running = node("alice", runtime).await;
    let log = running.data_dir().join("logs/node.jsonl");
    assert!(
        !log.exists(),
        "a node that has nothing to say creates no log"
    );

    let malformed = AccessConfig {
        public_host: "not a host".into(),
        team_domain: "x".into(),
        audience: "y".into(),
        owner_email: "alice@example.com".into(),
        app_id: None,
    };
    running
        .node
        .update_config(|config| config.access = Some(malformed))
        .unwrap();
    let _ = running.app();

    let lines: Vec<serde_json::Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["level"], "warn");
    assert_eq!(lines[0]["event"], "access");
    assert!(lines[0]["message"]
        .as_str()
        .unwrap()
        .starts_with("the access setting is malformed"));
    assert!(
        lines[0]["ts"].as_str().unwrap().starts_with(&T0[..13]),
        "the time is the node's clock: {}",
        lines[0]["ts"]
    );
}

#[tokio::test]
async fn a_refused_access_request_is_in_the_log_without_the_request() {
    let running = common::access::behind_access(None).await;
    let response = reqwest::Client::new()
        .get(format!("{}/api/me?secret=1", running.url()))
        .header("cf-ray", "1")
        .header("authorization", "Bearer s3cret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let text = std::fs::read_to_string(running.data_dir().join("logs/node.jsonl")).unwrap();
    let line: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(line["level"], "warn");
    assert_eq!(line["event"], "access");
    assert_eq!(
        line["message"],
        "refused an Access request (no Access token)"
    );
    assert!(!text.contains("s3cret") && !text.contains("secret=1"));
}

#[tokio::test]
async fn a_logging_setting_out_of_range_stops_the_node_and_names_the_setting() {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    let mut config = NodeConfig::new("http://127.0.0.1:1", "127.0.0.1:1", account, "scrum");
    config.logging.max_files = 0;
    data.save_config(&config).unwrap();
    let error = match agency_node::open(data, Overrides::default()) {
        Ok(_) => panic!("the node started"),
        Err(error) => error,
    };
    assert_eq!(error.to_string(), "logging.maxFiles must be 1 to 1000");
}

#[test]
fn the_logging_section_is_left_out_of_the_file_until_it_is_set() {
    let account = Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    let mut config = NodeConfig::new("http://127.0.0.1:1", "127.0.0.1:1", account, "scrum");
    assert!(!serde_json::to_string(&config).unwrap().contains("logging"));
    config.logging.max_age_days = 3;
    let text = serde_json::to_string(&config).unwrap();
    assert!(
        text.contains(r#""logging":{"level":"info","maxSizeMib":20,"maxAgeDays":3,"maxFiles":50}"#),
        "{text}"
    );
    let back: NodeConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(back.logging.max_age_days, 3);
}
