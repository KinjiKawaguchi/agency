//! What another node can make this node hold, and what a copied signed read can make it do.
mod common;

use agency_adapters::peers::{read_headers, MAX_ANSWER_BYTES};
use agency_core::orgs;
use agency_core::read_auth::sign_read;
use agency_core::views::{refresh, refresh_now, Replica};
use agency_proto::org::TeamPlace;
use common::{alice, id, joined_bob, Running};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SAMPLE: &str = include_str!("../../../testdata/teamview.sample.json");

/// How the fake team node answers.
#[derive(Clone)]
enum Answer {
    Json(String),
    /// A body of this many bytes, sent in chunks with no length up front.
    Chunked(usize),
}

/// A node at some origin that answers every request as `answer` says, over plain HTTP/1.1.
async fn fake_node(answer: Arc<Mutex<Answer>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let answer = answer.lock().unwrap().clone();
            tokio::spawn(async move {
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request).await;
                let head =
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nconnection: close\r\n";
                let _ = match answer {
                    Answer::Json(body) => {
                        let text = format!("{head}content-length: {}\r\n\r\n{body}", body.len());
                        socket.write_all(text.as_bytes()).await
                    }
                    Answer::Chunked(total) => {
                        let _ = socket
                            .write_all(
                                format!("{head}transfer-encoding: chunked\r\n\r\n").as_bytes(),
                            )
                            .await;
                        let chunk = vec![b' '; 64 * 1024];
                        let mut sent = 0;
                        while sent < total {
                            let part = &chunk[..chunk.len().min(total - sent)];
                            let framed =
                                [format!("{:x}\r\n", part.len()).as_bytes(), part, b"\r\n"]
                                    .concat();
                            if socket.write_all(&framed).await.is_err() {
                                break;
                            }
                            sent += part.len();
                        }
                        socket.write_all(b"0\r\n\r\n").await
                    }
                };
            });
        }
    });
    url
}

fn view_of_far(edit: impl FnOnce(&mut Value)) -> String {
    let mut view: Value = serde_json::from_str(SAMPLE).unwrap();
    view["team"] = json!("far");
    edit(&mut view);
    json!({ "view": view }).to_string()
}

/// The fake team node's answer with these `answersFor` entries next to the view.
fn answering_for(entries: Vec<Value>) -> String {
    let mut answer: Value = serde_json::from_str(&view_of_far(|_| {})).unwrap();
    answer["answersFor"] = Value::Array(entries);
    answer.to_string()
}

fn replica(alice: &Running) -> Replica {
    alice
        .node
        .store()
        .load_replica(&id("acme"), &id("far"))
        .unwrap()
}

#[tokio::test]
async fn an_answer_too_long_or_beyond_the_view_limits_is_not_kept() {
    let (alice, _) = alice().await;
    let answer = Arc::new(Mutex::new(Answer::Json(view_of_far(|_| {}))));
    let far = fake_node(answer.clone()).await;
    let place = TeamPlace::new(id("far"), "Far", far);
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();

    refresh(&alice.node).await.unwrap();
    let kept = replica(&alice);
    assert_eq!(kept.error, None);
    assert_eq!(kept.view.as_ref().unwrap().requests.len(), 3);

    let cases = [
        ("longer than", Answer::Chunked(MAX_ANSWER_BYTES + 1)),
        (
            "longer than",
            Answer::Json(view_of_far(|view| {
                view["label"] = json!("x".repeat(MAX_ANSWER_BYTES));
            })),
        ),
        (
            "beyond the limits",
            Answer::Json(view_of_far(|view| {
                view["requests"][0]["title"] = json!("x".repeat(201));
            })),
        ),
        (
            "beyond the limits",
            Answer::Json(answering_for(
                (0..201)
                    .map(|n| json!({ "name": format!("seat-{n}"), "id": "carol" }))
                    .collect(),
            )),
        ),
        (
            "beyond the limits",
            Answer::Json(answering_for(vec![
                json!({ "name": "x".repeat(201), "id": "carol" }),
            ])),
        ),
    ];
    for (expected, sent) in cases {
        *answer.lock().unwrap() = sent;
        // Read at once: after a failed read, the next pass would wait (see replica_backoff.rs).
        refresh_now(&alice.node).await.unwrap();
        let now = replica(&alice);
        assert!(
            now.error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{expected}: {:?}",
            now.error
        );
        assert_eq!(now.view, kept.view, "{expected}: the previous view stays");
    }
}

#[tokio::test]
async fn a_signed_read_is_accepted_once() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = "/fed/v1/teams/acme/app/view";
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", path, bob.node.now());
    let send = || {
        let mut request = reqwest::Client::new().get(format!("{}{path}", alice.url()));
        for (name, value) in read_headers(&auth) {
            request = request.header(name, value);
        }
        request.send()
    };
    assert_eq!(send().await.unwrap().status(), 200);
    let again = send().await.unwrap();
    assert_eq!(again.status(), 404, "a copy of the same read");
    assert_eq!(again.text().await.unwrap(), r#"{"error":"not found"}"#);

    bob.clock
        .advance(agency_proto::clock::Duration::milliseconds(1));
    let next = sign_read(bob.node.key(), &id("bob"), "GET", path, bob.node.now());
    assert_ne!(next.date, auth.date, "signed times carry milliseconds");
    let mut request = reqwest::Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in read_headers(&next) {
        request = request.header(name, value);
    }
    assert_eq!(request.send().await.unwrap().status(), 200, "a new read");
}
