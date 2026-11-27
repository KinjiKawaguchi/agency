//! `localClients`: a node on a machine that other accounts log in to refuses every direct local request to
//! the Client API and the web client, and keeps answering federation, which is checked by signatures.
mod common;

use agency_adapters::files::DataDir;
use agency_adapters::peers::read_headers;
use agency_core::config::NodeConfig;
use agency_core::ports::ConfigStore;
use agency_core::read_auth::sign_read;
use agency_core::routes;
use common::{alice, body, get, id, joined_bob, Running, TOKEN};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

fn refuse_local_clients(running: &Running) {
    running
        .node
        .update_config(|config| config.local_clients = false)
        .unwrap();
}

/// A write over loopback with the page's token: everything a local process can get.
async fn local_write(running: &Running) -> (StatusCode, Value) {
    let token = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    body(
        Client::new()
            .post(format!("{}/api/push/tokens", running.url()))
            .header("x-agency-token", TOKEN)
            .json(&token)
            .send()
            .await
            .unwrap(),
    )
    .await
}

/// bob's signed federation read of the org's `about` on `running`.
async fn federated_about(running: &Running, bob: &Running) -> StatusCode {
    let path = routes::about(&id("acme"));
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", &path, running.node.now());
    let mut request = Client::new().get(format!("{}{path}", running.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    request.send().await.unwrap().status()
}

#[tokio::test]
async fn a_node_that_refuses_local_clients_answers_no_local_read_page_or_write() {
    let (alice, _) = alice().await;
    let port = alice.port;
    assert_eq!(get(&alice, "/api/me").send().await.unwrap().status(), 200);

    refuse_local_clients(&alice);
    for host in [format!("127.0.0.1:{port}"), format!("localhost:{port}")] {
        let (status, answer) = body(
            get(&alice, "/api/me")
                .header("host", host.clone())
                .send()
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{host}: {answer}");
    }
    let page = get(&alice, "/").send().await.unwrap();
    assert_eq!(page.status(), StatusCode::FORBIDDEN);
    assert!(
        !page.text().await.unwrap().contains(TOKEN),
        "the write token is not served"
    );
    let (status, answer) = local_write(&alice).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{answer}");
    assert!(alice.node.store().push_tokens().is_empty());
}

#[tokio::test]
async fn a_node_that_admits_local_clients_lets_a_local_write_through() {
    let (alice, _) = alice().await;
    alice
        .node
        .update_config(|config| config.local_clients = true)
        .unwrap();
    let (status, answer) = local_write(&alice).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
}

#[tokio::test]
async fn federation_answers_whether_or_not_local_clients_are_admitted() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    assert_eq!(federated_about(&alice, &bob).await, StatusCode::OK);
    refuse_local_clients(&alice);
    // A later read, so that it is not refused as a copy of the first.
    alice
        .clock
        .advance(agency_proto::clock::Duration::seconds(1));
    assert_eq!(
        federated_about(&alice, &bob).await,
        StatusCode::OK,
        "a signed read from loopback is federation, not a local client"
    );
}

#[test]
fn the_setting_is_written_only_when_it_refuses_and_read_as_admitting_when_absent() {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    let config = |local_clients| {
        NodeConfig::new(
            "http://127.0.0.1:4400",
            "127.0.0.1:4400",
            agency_core::config::Account {
                id: id("alice"),
                label: "Alice".into(),
            },
            "scrum",
        )
        .with_local_clients(local_clients)
    };
    data.save_config(&config(true)).unwrap();
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(data.config_path()).unwrap()).unwrap();
    assert!(written.get("localClients").is_none(), "{written}");
    assert!(data.load_config().unwrap().local_clients);

    data.save_config(&config(false)).unwrap();
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(data.config_path()).unwrap()).unwrap();
    assert_eq!(written["localClients"], json!(false));
    assert!(!data.load_config().unwrap().local_clients);
}
