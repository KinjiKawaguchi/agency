//! What a client reports of its own calls: the owner's clients send it, the node keeps what it knows as
//! `client` series, and the routes a client may name are one file the clients read too.
mod common;

use agency_adapters::client_api::{reportable_routes, route_templates};
use agency_proto::metrics::{Metrics, Series};
use agency_proto::telemetry::{TelemetryAnswer, MOST_REPORT_BYTES, MOST_SAMPLES};
use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, Running, TOKEN};
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use serde_json::{json, Value};

fn sample(route: &str, status: &str, ms: Value) -> Value {
    json!({ "route": route, "status": status, "ms": ms })
}

fn send(running: &Running, body: &Value) -> RequestBuilder {
    Client::new()
        .post(format!("{}/api/telemetry", running.url()))
        .header("x-agency-token", TOKEN)
        .json(body)
}

async fn answer_of(response: reqwest::Response) -> TelemetryAnswer {
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}

async fn client_series(running: &Running) -> Vec<Series> {
    let response = Client::new()
        .get(format!("{}/api/metrics?kind=client", running.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json::<Metrics>().await.unwrap().series
}

#[test]
fn the_file_the_clients_read_is_the_table_of_routes_a_client_may_name() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/route-templates.json"
    );
    let made = route_templates();
    if std::env::var("AGENCY_WRITE_FIXTURES").is_ok_and(|value| value == "1") {
        std::fs::write(path, &made).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        made,
        "testdata/route-templates.json is not what the routes declare; run AGENCY_WRITE_FIXTURES=1 cargo test -p agency-node --test telemetry and commit the file"
    );
    let file: Value = serde_json::from_str(&made).unwrap();
    assert_eq!(
        file["routes"].as_array().unwrap().len(),
        reportable_routes().len()
    );
}

#[test]
fn a_route_is_declared_once() {
    let mut routes = agency_adapters::client_api::routes_behind_the_guard();
    let count = routes.len();
    routes.sort_unstable();
    routes.dedup();
    assert_eq!(routes.len(), count, "a route is declared twice");
}

#[tokio::test]
async fn every_route_a_client_may_name_is_a_route_with_that_method_on_the_router() {
    let (alice, _) = alice().await;
    for entry in reportable_routes() {
        let (method, template) = entry.split_once(' ').unwrap();
        let path = template.replace(['{', '}'], "");
        let response = Client::new()
            .request(
                Method::from_bytes(method.as_bytes()).unwrap(),
                format!("{}{path}", alice.url()),
            )
            .header("x-agency-token", TOKEN)
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body = response.text().await.unwrap();
        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{entry}: the route does not take this method"
        );
        assert!(
            !body.contains("no such route"),
            "{entry}: the router has no such route ({status}: {body})"
        );
    }
}

#[tokio::test]
async fn the_owner_reports_and_the_node_keeps_the_known_routes_by_client_and_counts_the_rest() {
    let (alice, _) = alice().await;
    let report = json!({ "client": "web", "samples": [
        sample("GET /api/me", "2xx", json!(12)),
        sample("GET /api/me", "2xx", json!(40)),
        sample("GET /api/orgs/{org}/teams/{team}/view", "network", json!(300)),
        sample("GET /api/orgs/acme/teams/app/view", "2xx", json!(5)),
        sample("GET /api/me", "6xx", json!(5)),
        sample("GET /api/me", "ok", json!(5)),
        sample("GET /api/me?x=1", "2xx", json!(5)),
        sample("POST /api/telemetry", "2xx", json!(5)),
        sample("GET /fed/v1/orgs/{org}/about", "2xx", json!(5)),
    ]});
    let answer = answer_of(send(&alice, &report).send().await.unwrap()).await;
    assert_eq!((answer.accepted, answer.dropped), (3, 6));
    answer_of(
        send(
            &alice,
            &json!({ "client": "ios", "samples": [
                sample("GET /api/me", "2xx", json!(7)),
            ]}),
        )
        .send()
        .await
        .unwrap(),
    )
    .await;

    let series = client_series(&alice).await;
    assert!(series.iter().all(Series::is_consistent), "{series:?}");
    let seen: Vec<(&str, &str, &str, u64, u64, &str)> = series
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.status.as_str(),
                s.client.as_deref().unwrap(),
                s.count,
                s.errors,
                s.group.as_str(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            ("GET /api/me", "2xx", "ios", 1, 0, "desk"),
            ("GET /api/me", "2xx", "web", 2, 0, "desk"),
            (
                "GET /api/orgs/{org}/teams/{team}/view",
                "network",
                "web",
                1,
                1,
                "teams"
            ),
        ]
    );
    let web_me = series
        .iter()
        .find(|s| s.name == "GET /api/me" && s.client.as_deref() == Some("web"))
        .unwrap();
    assert_eq!(web_me.sum_ms, 52, "the milliseconds the client measured");
}

#[tokio::test]
async fn a_wait_longer_than_the_last_bound_is_in_the_last_bucket_whatever_the_client_measured() {
    let (alice, _) = alice().await;
    let report = json!({ "client": "web", "samples": [
        sample("GET /api/me", "2xx", json!(u64::MAX)),
        sample("GET /api/me", "2xx", json!(30_000)),
    ]});
    answer_of(send(&alice, &report).send().await.unwrap()).await;
    let series = client_series(&alice).await;
    let me = &series[0];
    assert_eq!(me.count, 2);
    assert_eq!(
        me.sum_ms,
        30_000 + 30_001,
        "a sample counts as 30 001 at most"
    );
    assert_eq!(me.buckets[13], 1, "30 000 is in the bucket it bounds");
    assert_eq!(me.buckets[14], 1, "longer is in the last bucket");
    assert_eq!(me.p95_ms, None, "slower than 30 000 ms has no percentile");
}

#[tokio::test]
async fn a_report_that_is_not_well_formed_is_refused_whole_and_records_nothing() {
    let (alice, _) = alice().await;
    let too_many: Vec<Value> = (0..=MOST_SAMPLES)
        .map(|_| sample("GET /api/me", "2xx", json!(1)))
        .collect();
    let bad = [
        json!({ "client": "desktop", "samples": [] }),
        json!({ "client": "WEB", "samples": [] }),
        json!({ "samples": [] }),
        json!({ "client": "web" }),
        json!({ "client": "web", "samples": too_many }),
        json!({ "client": "web", "samples": [sample("GET /api/me", "2xx", json!(-1))] }),
        json!({ "client": "web", "samples": [sample("GET /api/me", "2xx", json!(1.5))] }),
        json!({ "client": "web", "samples": [sample("GET /api/me", "2xx", json!("12"))] }),
        json!({ "client": "web", "samples": [{ "route": "GET /api/me", "status": "2xx" }] }),
        json!({ "client": "web", "samples": [{ "route": 1, "status": "2xx", "ms": 1 }] }),
        json!([]),
    ];
    for body in &bad {
        let response = send(&alice, body).send().await.unwrap();
        assert_eq!(response.status(), 400, "{body}");
    }
    let garbled = Client::new()
        .post(format!("{}/api/telemetry", alice.url()))
        .header("x-agency-token", TOKEN)
        .header("content-type", "application/json")
        .body("{\"client\":")
        .send()
        .await
        .unwrap();
    assert_eq!(garbled.status(), 400);
    assert!(
        client_series(&alice).await.is_empty(),
        "a refused report leaves nothing"
    );
}

#[tokio::test]
async fn the_most_samples_and_an_empty_report_are_accepted() {
    let (alice, _) = alice().await;
    let most: Vec<Value> = (0..MOST_SAMPLES)
        .map(|_| sample("GET /api/me", "2xx", json!(1)))
        .collect();
    let answer = answer_of(
        send(&alice, &json!({ "client": "web", "samples": most }))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((answer.accepted, answer.dropped), (MOST_SAMPLES as u64, 0));
    let empty = answer_of(
        send(&alice, &json!({ "client": "web", "samples": [] }))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((empty.accepted, empty.dropped), (0, 0));
}

#[tokio::test]
async fn a_report_over_the_most_bytes_is_refused_and_one_at_the_limit_is_read() {
    let (alice, _) = alice().await;
    let padded = |bytes: usize| {
        let head = r#"{"client":"web","samples":[],"pad":""#;
        let tail = r#""}"#;
        let fill = bytes - head.len() - tail.len();
        format!("{head}{}{tail}", "x".repeat(fill))
    };
    let post = |body: String| {
        Client::new()
            .post(format!("{}/api/telemetry", alice.url()))
            .header("x-agency-token", TOKEN)
            .header("content-type", "application/json")
            .body(body)
            .send()
    };
    assert_eq!(
        post(padded(MOST_REPORT_BYTES + 1)).await.unwrap().status(),
        413
    );
    assert_eq!(post(padded(MOST_REPORT_BYTES)).await.unwrap().status(), 200);
}

#[tokio::test]
async fn it_is_a_write_like_the_others_with_the_write_token_json_and_the_owner() {
    let (alice, _) = alice().await;
    let report = json!({ "client": "web", "samples": [sample("GET /api/me", "2xx", json!(1))] });
    let no_token = Client::new()
        .post(format!("{}/api/telemetry", alice.url()))
        .json(&report)
        .send()
        .await
        .unwrap();
    assert_eq!(no_token.status(), 403);
    let wrong_token = Client::new()
        .post(format!("{}/api/telemetry", alice.url()))
        .header("x-agency-token", "nope")
        .json(&report)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_token.status(), 403);
    let not_json = Client::new()
        .post(format!("{}/api/telemetry", alice.url()))
        .header("x-agency-token", TOKEN)
        .header("content-type", "text/plain")
        .body(report.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(not_json.status(), 415);
    let get = Client::new()
        .get(format!("{}/api/telemetry", alice.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 405);
    assert!(client_series(&alice).await.is_empty());
}

#[tokio::test]
async fn through_cloudflare_only_the_owner_reports() {
    let running = behind_access(None).await;
    let report = json!({ "client": "web", "samples": [sample("GET /api/me", "2xx", json!(1))] });
    let through = |email: &str| {
        send(&running, &report)
            .header("host", PUBLIC_HOST)
            .header("cf-ray", "1")
            .header("cf-access-jwt-assertion", access_token(email, |_| {}))
    };
    assert_eq!(
        through("alice@example.com").send().await.unwrap().status(),
        200
    );
    for other in ["carol@example.com", "bob@example.com"] {
        assert_eq!(
            through(other).send().await.unwrap().status(),
            403,
            "{other}: someone Access lets in, but not the owner"
        );
    }
    assert_eq!(
        send(&running, &report).send().await.unwrap().status(),
        403,
        "over loopback, a node behind Access does not write as the owner"
    );
}

#[tokio::test]
async fn the_call_that_reports_is_timed_as_a_route_of_the_node_and_is_not_one_a_client_may_name() {
    let (alice, _) = alice().await;
    let report = json!({ "client": "web", "samples": [] });
    answer_of(send(&alice, &report).send().await.unwrap()).await;
    let metrics: Metrics = Client::new()
        .get(format!("{}/api/metrics?kind=serve", alice.url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let own = metrics
        .series
        .iter()
        .find(|s| s.name == "POST /api/telemetry")
        .expect("the node times the call like any other");
    assert_eq!(own.group, "metrics");
    assert!(!reportable_routes().contains(&"POST /api/telemetry".to_owned()));
}
