//! On a node behind Access, the owner is who Access says it is: a process on the node's machine reads
//! over loopback but cannot write as the owner.
mod common;

use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{get, Running, TOKEN};
use serde_json::{json, Value};

fn push_token() -> Value {
    json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" })
}

/// A write over loopback with the page's token: everything a local process can get.
async fn local_write(running: &Running) -> (u16, Value) {
    let response = reqwest::Client::new()
        .post(format!("{}/api/push/tokens", running.url()))
        .header("x-agency-token", TOKEN)
        .json(&push_token())
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn owner_write(running: &Running) -> u16 {
    reqwest::Client::new()
        .post(format!("{}/api/push/tokens", running.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header(
            "cf-access-jwt-assertion",
            access_token("alice@example.com", |_| {}),
        )
        .header("x-agency-token", TOKEN)
        .json(&push_token())
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn behind_access_loopback_reads_but_writes_only_through_access() {
    let running = behind_access(None).await;
    let read = get(&running, "/api/me").send().await.unwrap().status();
    assert_eq!(read, 200, "a local read is still let in");

    let (status, answer) = local_write(&running).await;
    assert_eq!(status, 403, "{answer}");
    assert!(
        answer["error"].as_str().unwrap().contains("behind Access"),
        "{answer}"
    );
    let delete = reqwest::Client::new()
        .delete(format!(
            "{}/api/push/tokens/{}",
            running.url(),
            "ab12".repeat(16)
        ))
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 403, "a DELETE is a write too");

    assert_eq!(
        owner_write(&running).await,
        200,
        "the owner, through Access"
    );
}

#[tokio::test]
async fn a_malformed_access_setting_does_not_make_loopback_the_owner_again() {
    let mut running = behind_access(None).await;
    running
        .node
        .update_config(|config| {
            if let Some(access) = config.access.as_mut() {
                access.audience = "not an audience".into();
            }
        })
        .unwrap();
    running.stop().await;
    running.start().await;
    let (status, answer) = local_write(&running).await;
    assert_eq!(status, 403, "{answer}");
}

#[tokio::test]
async fn without_access_the_machine_is_the_owner() {
    let (running, _) = common::alice().await;
    let (status, answer) = local_write(&running).await;
    assert_eq!(status, 200, "{answer}");
}
