//! What a node says about itself: to an app over the Client API, and to a member of an org over a signed
//! federation read. A stranger learns nothing, not even the version.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::about::{this_node, NODE_VERSION};
use agency_core::read_auth::sign_read;
use agency_core::routes;
use agency_proto::about::{About, Capability};
use agency_proto::{AccountKey, Id};
use common::{alice, body, get, id, joined_bob, Running};
use reqwest::{Client, StatusCode};
use serde_json::Value;

/// A federation read of alice's `about` of `org`, signed by `key` as `account`, for `signed_path`.
async fn federated(
    alice: &Running,
    key: &AccountKey,
    account: &Id,
    org: &str,
    signed_path: &str,
) -> (StatusCode, Value) {
    let auth = sign_read(key, account, "GET", signed_path, alice.node.now());
    let mut request = Client::new().get(format!("{}/fed/v1/orgs/{org}/about", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn an_app_reads_the_release_the_contracts_and_the_capabilities() {
    let (alice, _) = alice().await;
    let (status, answer) = body(get(&alice, "/api/about").send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let about: About = serde_json::from_value(answer.clone()).unwrap();
    assert_eq!(about, this_node());
    assert_eq!(about.node, NODE_VERSION);
    assert_eq!(about.contracts.federation, [1]);
    assert_eq!(about.contracts.client_api.current, 1);
    for one in Capability::ALL {
        assert!(about.lists(one), "{one:?}");
    }
    assert!(
        answer.get("oldestApp").is_none(),
        "no floor is set, so none is said"
    );
}

#[tokio::test]
async fn a_member_reads_it_by_a_signed_read_of_the_org() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = routes::about(&id("acme"));
    let (status, answer) = federated(&alice, bob.node.key(), &id("bob"), "acme", &path).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        serde_json::from_value::<About>(answer.clone()).unwrap(),
        this_node()
    );
    assert!(
        answer.get("oldestApp").is_none(),
        "the federation answer has no oldestApp"
    );
}

#[tokio::test]
async fn a_stranger_learns_nothing_about_the_node_not_even_its_version() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = routes::about(&id("acme"));

    // No signature at all.
    let bare = Client::new()
        .get(format!("{}{path}", alice.url()))
        .send()
        .await
        .unwrap();
    let (bare_status, bare_body) = (bare.status(), bare.text().await.unwrap());
    // A signature by a key that is not the member's, for the member's account.
    let stranger = AccountKey::from_seed([9; 32]);
    let (forged_status, forged) = federated(&alice, &stranger, &id("bob"), "acme", &path).await;
    // A member's signature for another path.
    let other = routes::definition(&id("acme"));
    let (moved_status, _) = federated(&alice, bob.node.key(), &id("bob"), "acme", &other).await;
    // An org this node does not have.
    let missing = routes::about(&id("nope"));
    let (missing_status, missing_body) =
        federated(&alice, bob.node.key(), &id("bob"), "nope", &missing).await;

    for status in [bare_status, forged_status, moved_status, missing_status] {
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    assert!(!bare_body.contains(NODE_VERSION), "{bare_body}");
    assert_eq!(
        forged, missing_body,
        "an org that is not there looks like a read that is refused"
    );
}

#[tokio::test]
async fn a_copy_of_a_read_that_was_already_answered_is_refused() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = routes::about(&id("acme"));
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", &path, alice.node.now());
    let send = || {
        let mut request = Client::new().get(format!("{}{path}", alice.url()));
        for (name, value) in read_headers(&auth) {
            request = request.header(name, value);
        }
        request.send()
    };
    assert_eq!(send().await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        send().await.unwrap().status(),
        StatusCode::NOT_FOUND,
        "the same read, sent again"
    );
}
