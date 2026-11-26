//! A member's node that missed versions of an org definition takes them in order, each checked.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::departments;
use agency_core::devices::{add_device, Added};
use agency_core::read_auth::sign_read;
use agency_core::routes;
use agency_core::views::refresh;
use agency_proto::org::Department;
use agency_proto::AccountKey;
use common::{alice, id, joined_bob, Running};
use serde_json::Value;

fn version(running: &Running) -> u64 {
    running
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version
}

/// The root writes `count` versions that bob's node does not see.
fn write_versions(alice: &Running, count: usize) {
    for n in 0..count {
        let department = Department {
            id: id(&format!("dept-{n}")),
            label: format!("Department {n}"),
            parent: None,
            head: None,
        };
        departments::add(&alice.node, &id("acme"), department).unwrap();
    }
}

#[tokio::test]
async fn a_member_that_missed_versions_takes_them_in_order() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    write_versions(&alice, 25);
    assert!(version(&alice) >= version(&bob) + 25);

    refresh(&bob.node).await.unwrap();
    assert_eq!(
        version(&bob),
        version(&alice),
        "more than one answer's worth of versions"
    );
    let theirs = alice.node.store().load_org(&id("acme")).unwrap();
    let ours = bob.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(ours.digest(), theirs.digest());
    for older in 1..version(&bob) {
        let kept = bob.node.store().load_org_version(&id("acme"), older);
        if older >= 3 {
            assert!(kept.is_some(), "version {older} is in bob's history");
        }
    }
}

#[tokio::test]
async fn a_device_added_after_missed_versions_is_kept() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    write_versions(&alice, 3);
    let laptop = AccountKey::generate();
    let added = add_device(&bob.node, "Laptop", &laptop.public(), None).await;
    assert!(matches!(&added[..], [(_, Added::Done(_))]), "{added:?}");
    assert_eq!(version(&bob), version(&alice));
}

async fn versions(alice: &Running, reader: &Running, account: &str, after: u64) -> (u16, Value) {
    let path = routes::versions(&id("acme"), after);
    let auth = sign_read(
        reader.node.key(),
        &id(account),
        "GET",
        &path,
        reader.node.now(),
    );
    let mut request = reqwest::Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_versions_route_answers_members_a_bounded_run_of_versions() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    write_versions(&alice, 25);
    let (status, answer) = versions(&alice, &bob, "bob", 1).await;
    assert_eq!(status, 200);
    let numbers: Vec<u64> = answer["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|doc| doc["body"]["version"].as_u64().unwrap())
        .collect();
    assert_eq!(
        numbers,
        (2..=21).collect::<Vec<_>>(),
        "oldest first, at most 20"
    );
    bob.clock
        .advance(agency_proto::clock::Duration::milliseconds(1));
    let (_, answer) = versions(&alice, &bob, "bob", version(&alice)).await;
    assert_eq!(
        answer["versions"],
        serde_json::json!([]),
        "nothing after the newest"
    );

    let stranger = common::node("carol", common::memory_runtime().0).await;
    let (status, answer) = versions(&alice, &stranger, "carol", 1).await;
    assert_eq!(
        (status, answer),
        (404, serde_json::json!({ "error": "not found" }))
    );
}

#[tokio::test]
async fn a_version_not_signed_by_the_root_stops_the_catch_up() {
    use agency_proto::{Kind, OrgDefinition, Signed};
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let current = bob.node.store().load_org(&id("acme")).unwrap();
    let mallory = AccountKey::generate();
    let mut body = OrgDefinition::successor_of(&current, bob.node.now_text());
    body.label = "Taken over".into();
    let forged = Signed::sign(Kind::Org, body, id("alice"), &mallory).unwrap();
    let answer = serde_json::json!({ "versions": [forged] }).to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fake = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().fallback(move || {
        let answer = answer.clone();
        async move { ([("content-type", "application/json")], answer) }
    });
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let target = current.body.version + 5;
    let caught = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        agency_core::orgs::catch_up(&bob.node, &fake, &id("acme"), target),
    )
    .await
    .expect("the catch-up ends");
    assert!(caught.is_err(), "{caught:?}");
    let kept = bob.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(kept.digest(), current.digest(), "nothing was taken");
}

/// A node that answers every versions read with `answer`.
async fn answering(answer: String) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().fallback(move || {
        let answer = answer.clone();
        async move { ([("content-type", "application/json")], answer) }
    });
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn an_answer_that_moves_nothing_on_ends_the_catch_up() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let current = bob.node.store().load_org(&id("acme")).unwrap();
    let stale = answering(serde_json::json!({ "versions": [current.clone()] }).to_string()).await;
    let target = current.body.version + 5;
    let caught = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        agency_core::orgs::catch_up(&bob.node, &stale, &id("acme"), target),
    )
    .await
    .expect("the catch-up ends");
    assert!(caught.is_err(), "{caught:?}");
}
