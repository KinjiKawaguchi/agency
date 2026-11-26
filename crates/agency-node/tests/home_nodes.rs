//! The owner's other home nodes: configured on the node, given to the owner in `/api/me`, and to no one
//! else.
mod common;

use agency_core::config::HomeNodeConfig;
use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, get, Running};
use serde_json::{json, Value};

fn entry(label: &str, url: &str) -> HomeNodeConfig {
    HomeNodeConfig::new(label, url)
}

fn configure(running: &Running) {
    let own = running.url();
    let entries = vec![
        entry("Work", "https://work.example.com/"),
        entry(" Lab ", "http://127.0.0.1:4500"),
        entry("", "https://nameless.example.com"),
        entry("Plain", "http://work.example.com"),
        entry("Me again", &own),
        entry("Twice", "https://work.example.com"),
        entry("Twice in capitals", "https://WORK.example.com:443"),
        entry("Me with a slash", &format!("{own}/")),
    ];
    running
        .node
        .update_config(|config| config.home_nodes = entries)
        .unwrap();
}

fn listed() -> Value {
    json!([
        { "label": "Work", "url": "https://work.example.com" },
        { "label": "Lab", "url": "http://127.0.0.1:4500" },
    ])
}

async fn me_through_access(running: &Running, email: &str) -> Value {
    reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(email, |_| {}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn the_owner_is_given_the_usable_home_nodes_and_no_one_else_any() {
    let local = alice().await.0;
    let me: Value = get(&local, "/api/me")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        me.get("homeNodes").is_none(),
        "absent when none are configured"
    );
    configure(&local);
    let me: Value = get(&local, "/api/me")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["homeNodes"], listed(), "a local request is the owner's");

    let public = behind_access(None).await;
    configure(&public);
    let owner = me_through_access(&public, "alice@example.com").await;
    assert_eq!(owner["homeNodes"], listed());
    let other = me_through_access(&public, "carol@example.com").await;
    assert!(other["account"].is_object(), "the rest of me is still read");
    assert!(
        other.get("homeNodes").is_none(),
        "another sign-in is not told where else the owner works"
    );
}

#[test]
fn entries_that_cannot_be_used_are_left_out_with_a_reason() {
    let mut config: agency_core::config::NodeConfig = serde_json::from_value(json!({
        "url": "http://Localhost:4400/", "listen": "127.0.0.1:4400",
        "account": { "id": "alice", "label": "Alice" }, "scrum": "scrum",
        "homeNodes": [{ "label": "Work", "url": "https://work.example.com" }]
    }))
    .unwrap();
    config.home_nodes.push(entry("Me", "http://localhost:4400"));
    config
        .home_nodes
        .push(entry("Far", "https://far.example.com/path"));
    let (kept, left_out) = agency_core::client::home_nodes(&config);
    assert_eq!(kept.len(), 1);
    assert_eq!(left_out.len(), 2, "{left_out:?}");
    assert!(left_out[0].contains("it is this node"));
    assert!(left_out[1].contains("no path"));
    assert!(
        serde_json::from_value::<agency_core::config::NodeConfig>(json!({
            "url": "http://127.0.0.1:4400", "listen": "127.0.0.1:4400",
            "account": { "id": "alice", "label": "Alice" }, "scrum": "scrum",
            "homeNodes": [{ "label": "Work", "url": "https://work.example.com", "token": "x" }]
        }))
        .is_err(),
        "an entry holds a label and an origin, nothing else"
    );
}
