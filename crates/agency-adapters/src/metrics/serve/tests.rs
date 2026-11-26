use super::*;
use crate::memory::ManualClock;
use agency_proto::metrics::Series;
use axum::routing::{any, get};
use axum::{middleware, Router};
use std::sync::Arc;

/// A server with the middleware over a few routes, and the recorder it records in.
async fn server() -> (String, Recorder, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let recorder = Recorder::new(
        dir.path().join("metrics"),
        Arc::new(ManualClock::at("2026-11-24T11:30:00Z")),
    );
    let app = Router::new()
        .route(
            "/api/orgs/{org}/teams/{team}/usage",
            get(|| async { "usage" }),
        )
        .route("/boom", get(|| async { StatusCode::INTERNAL_SERVER_ERROR }))
        .route("/moved", get(|| async { StatusCode::FOUND }))
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                "slow"
            }),
        )
        .route("/any", any(|| async { "any" }))
        .layer(middleware::from_fn_with_state(
            recorder.clone(),
            record_request,
        ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, recorder, dir)
}

fn summary(recorder: &Recorder) -> Vec<(String, String, u64, u64)> {
    recorder
        .read(1, None, None)
        .series
        .iter()
        .map(|s| (s.name.clone(), s.status.clone(), s.count, s.errors))
        .collect()
}

#[tokio::test]
async fn a_request_is_named_by_the_template_it_matched_and_never_by_its_path_or_query() {
    let (url, recorder, _dir) = server().await;
    let client = reqwest::Client::new();
    for path in [
        "/api/orgs/acme/teams/app/usage?token=secret&email=alice@example.com",
        "/api/orgs/other/teams/web/usage",
        "/api/orgs/acme/teams/app/usage",
    ] {
        assert_eq!(
            client
                .get(format!("{url}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    assert_eq!(
        summary(&recorder),
        [(
            "GET /api/orgs/{org}/teams/{team}/usage".to_owned(),
            "2xx".to_owned(),
            3,
            0
        )],
        "three paths, one series, and nothing of the ids, the token, or the email"
    );
}

#[tokio::test]
async fn every_request_that_matched_no_route_is_one_series_whatever_it_asked_for() {
    let (url, recorder, _dir) = server().await;
    let client = reqwest::Client::new();
    for path in [
        "/wp-admin/setup.php",
        "/.env",
        "/api/orgs/acme/teams/app/nothing",
        "/a/b/c/d",
    ] {
        assert_eq!(
            client
                .get(format!("{url}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    let method = reqwest::Method::from_bytes(b"BREW").unwrap();
    client
        .request(method, format!("{url}/coffee"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        summary(&recorder),
        [("unmatched".to_owned(), "4xx".to_owned(), 5, 0)]
    );
}

#[tokio::test]
async fn a_method_outside_the_standard_set_is_other_and_the_standard_ones_are_named() {
    let (url, recorder, _dir) = server().await;
    let client = reqwest::Client::new();
    for method in ["PROPFIND", "BREW", "TRACE"] {
        let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        client
            .request(method, format!("{url}/any"))
            .send()
            .await
            .unwrap();
    }
    for method in [
        reqwest::Method::GET,
        reqwest::Method::POST,
        reqwest::Method::DELETE,
        reqwest::Method::OPTIONS,
    ] {
        client
            .request(method, format!("{url}/any"))
            .send()
            .await
            .unwrap();
    }
    let names: Vec<(String, u64)> = recorder
        .read(1, None, None)
        .series
        .iter()
        .map(|s| (s.name.clone(), s.count))
        .collect();
    assert_eq!(
        names,
        [
            ("DELETE /any".to_owned(), 1),
            ("GET /any".to_owned(), 1),
            ("OPTIONS /any".to_owned(), 1),
            ("OTHER /any".to_owned(), 3),
            ("POST /any".to_owned(), 1),
        ],
        "the three odd methods are one series, so a requester cannot make a series by choosing one"
    );
}

#[tokio::test]
async fn the_status_class_and_the_time_are_recorded_and_a_5xx_is_an_error() {
    let (url, recorder, _dir) = server().await;
    let client = reqwest::Client::new();
    client.get(format!("{url}/boom")).send().await.unwrap();
    client.get(format!("{url}/slow")).send().await.unwrap();
    client.get(format!("{url}/moved")).send().await.unwrap();
    let metrics = recorder.read(1, None, None);
    let by_name =
        |name: &str| -> &Series { metrics.series.iter().find(|s| s.name == name).unwrap() };
    assert_eq!(
        (
            by_name("GET /boom").status.as_str(),
            by_name("GET /boom").errors
        ),
        ("5xx", 1)
    );
    assert_eq!(
        by_name("GET /moved").status,
        "3xx",
        "a redirect that was not followed is a 3xx"
    );
    assert!(
        by_name("GET /slow").sum_ms >= 30,
        "{}",
        by_name("GET /slow").sum_ms
    );
    assert_eq!(by_name("GET /slow").errors, 0);
    assert!(metrics.series.iter().all(Series::is_consistent));
}

#[tokio::test]
async fn a_route_that_refuses_a_method_is_one_series_whatever_method_was_tried() {
    let (url, recorder, _dir) = server().await;
    let client = reqwest::Client::new();
    for method in ["POST", "PUT", "DELETE", "PATCH", "BREW"] {
        let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let response = client
            .request(method, format!("{url}/boom"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 405);
    }
    assert_eq!(
        summary(&recorder),
        [("OTHER /boom".to_owned(), "4xx".to_owned(), 5, 0)]
    );
}
