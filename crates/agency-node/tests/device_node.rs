//! Where and how to approve a device at the node's terminal (`node` of `GET /api/devices`): given to the
//! node's owner only, because the command holds the path of the running program.
mod common;

use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, body, get, Running};
use serde_json::{json, Value};

async fn devices_through_access(running: &Running, email: &str) -> Value {
    let request = get(running, "/api/devices")
        .header("host", PUBLIC_HOST)
        .header("cf-access-jwt-assertion", access_token(email, |_| {}));
    let (status, list) = body(request.send().await.unwrap()).await;
    assert_eq!(status, 200, "{list}");
    list
}

fn expected_node(url: &str) -> Value {
    json!({
        "url": url,
        "dataDir": "/var/lib/agency",
        "command": "/opt/agency/bin/agency --dir /var/lib/agency device-add",
    })
}

#[tokio::test]
async fn a_direct_local_request_is_given_the_node_and_its_command() {
    let (alice, _) = alice().await;
    let (_, list) = body(get(&alice, "/api/devices").send().await.unwrap()).await;
    assert_eq!(list["node"], expected_node(&alice.url()), "{list}");
}

#[tokio::test]
async fn through_access_only_the_owner_is_given_the_command() {
    let running = behind_access(None).await;
    let owner = devices_through_access(&running, "alice@example.com").await;
    assert_eq!(owner["node"], expected_node(&running.url()), "{owner}");

    let other = devices_through_access(&running, "carol@example.com").await;
    assert!(
        other.get("node").is_none(),
        "a signed-in email that is not the owner's reads the list without the node: {other}"
    );
    assert!(other["orgs"].is_array() && other["requests"].is_array());
}
