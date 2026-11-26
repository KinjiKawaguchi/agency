//! The node's own measurements: every route is timed by its template, and only the owner reads them.
mod common;

use agency_core::config::NodeConfig;
use agency_core::metrics_config::Objective;
use agency_core::ports::ConfigStore as _;
use agency_proto::metrics::{Metrics, ObjectiveState, Series};
use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, Running};
use reqwest::{Client, RequestBuilder, Response, StatusCode};

fn get(running: &Running, path: &str) -> RequestBuilder {
    Client::new().get(format!("{}{path}", running.url()))
}

/// A read through Cloudflare, signed in to Access as `email`.
fn through_access(running: &Running, path: &str, email: &str) -> RequestBuilder {
    get(running, path)
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(email, |_| {}))
}

async fn metrics_of(response: Response) -> Metrics {
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}

#[tokio::test]
async fn the_owner_reads_the_routes_timed_by_template_and_what_matched_no_route_as_one_series() {
    let (alice, _) = alice().await;
    for _ in 0..2 {
        get(&alice, "/api/me").send().await.unwrap();
    }
    get(&alice, "/api/orgs/acme/teams/app/view")
        .send()
        .await
        .unwrap();
    for path in ["/wp-admin/setup.php", "/.env"] {
        assert_eq!(
            get(&alice, path).send().await.unwrap().status(),
            StatusCode::OK,
            "the web client answers it as a page"
        );
    }
    let metrics = metrics_of(get(&alice, "/api/metrics").send().await.unwrap()).await;
    assert!(
        metrics.series.iter().all(Series::is_consistent),
        "{:?}",
        metrics.series
    );
    let names: Vec<(&str, &str, u64)> = metrics
        .series
        .iter()
        .map(|s| (s.name.as_str(), s.status.as_str(), s.count))
        .collect();
    assert!(names.contains(&("GET /api/me", "2xx", 2)), "{names:?}");
    assert!(
        names.contains(&("GET /api/orgs/{org}/teams/{team}/view", "2xx", 1)),
        "{names:?}"
    );
    assert!(
        names
            .iter()
            .any(|(name, _, count)| *name == "unmatched" && *count == 2),
        "{names:?}"
    );
    assert!(
        metrics.series.iter().all(|s| !s.name.contains("acme")
            && !s.name.contains("wp-admin")
            && !s.name.contains("env")),
        "no id and no path a scanner chose"
    );
    assert!(
        metrics.series.iter().all(|s| !s.by_hour.is_empty()),
        "the hours of a short range"
    );
}

#[tokio::test]
async fn a_read_can_be_narrowed_is_small_for_a_long_range_and_refuses_what_it_does_not_understand()
{
    let (alice, _) = alice().await;
    get(&alice, "/api/me").send().await.unwrap();
    get(&alice, "/api/about").send().await.unwrap();
    let one = metrics_of(
        get(&alice, "/api/metrics?kind=serve&name=GET%20/api/me")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(one.series.len(), 1);
    assert_eq!(one.series[0].name, "GET /api/me");
    assert!(
        metrics_of(get(&alice, "/api/metrics?kind=call").send().await.unwrap())
            .await
            .series
            .is_empty()
    );
    let week = metrics_of(get(&alice, "/api/metrics?days=7").send().await.unwrap()).await;
    assert!(week.series.iter().all(|s| !s.by_hour.is_empty()));
    let month = metrics_of(get(&alice, "/api/metrics?days=30").send().await.unwrap()).await;
    assert!(
        !month.series.is_empty() && month.series.iter().all(|s| s.by_hour.is_empty()),
        "totals only past 7 days"
    );
    for bad in [
        "days=0",
        "days=31",
        "days=x",
        "days=-1",
        "kind=bogus",
        "kind=",
    ] {
        let response = get(&alice, &format!("/api/metrics?{bad}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "{bad}");
    }
}

#[tokio::test]
async fn the_prometheus_text_is_read_on_the_nodes_own_machine_and_holds_no_id() {
    let (alice, _) = alice().await;
    get(&alice, "/api/orgs/acme/teams/app/view")
        .send()
        .await
        .unwrap();
    let response = get(&alice, "/metrics").send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let text = response.text().await.unwrap();
    assert!(text.contains(r#"agency_duration_milliseconds_count{kind="serve",name="GET /api/orgs/{org}/teams/{team}/view",status="2xx"} 1"#), "{text}");
    assert!(!text.contains("acme"));
}

#[tokio::test]
async fn through_cloudflare_only_the_owner_reads_the_measurements_and_nobody_reads_the_prometheus_text(
) {
    let running = behind_access(None).await;
    let owner = "alice@example.com";
    assert_eq!(
        through_access(&running, "/api/metrics", owner)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    for other in ["carol@example.com", "bob@example.com"] {
        let response = through_access(&running, "/api/metrics", other)
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            403,
            "{other}: someone Access lets read, but not the owner"
        );
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body["error"].as_str().unwrap().contains("owner"), "{body}");
    }
    for who in [owner, "carol@example.com"] {
        let response = through_access(&running, "/metrics", who)
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            403,
            "{who}: the text is read on the machine itself"
        );
    }
    // No sign-in at all, through Cloudflare, reads nothing either.
    let nobody = get(&running, "/api/metrics")
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1");
    assert_eq!(nobody.send().await.unwrap().status(), 403);
    // The machine itself, directly, reads both.
    assert_eq!(
        get(&running, "/api/metrics").send().await.unwrap().status(),
        200
    );
    assert_eq!(
        get(&running, "/metrics").send().await.unwrap().status(),
        200
    );
}

#[tokio::test]
async fn no_other_node_reads_them_the_federation_has_no_such_route() {
    let (alice, _) = alice().await;
    for path in [
        "/fed/v1/metrics",
        "/fed/v1/teams/acme/app/metrics",
        "/fed/v1/orgs/acme/metrics",
    ] {
        let response = get(&alice, path).send().await.unwrap();
        // A path that is no route is answered as any other: not found, or the web client's page.
        let kind = response
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap().to_owned());
        let text = response.text().await.unwrap();
        assert!(
            !text.contains("agency_duration") && !text.contains("\"series\""),
            "{path}: {text}"
        );
        assert!(
            kind.as_deref()
                .is_none_or(|kind| !kind.starts_with("text/plain")),
            "{path}: {kind:?}"
        );
    }
}

#[tokio::test]
async fn the_objectives_the_operator_set_are_judged_against_the_measurements_of_the_whole_group() {
    let (alice, _) = alice().await;
    alice
        .node
        .update_config(|config| {
            config.metrics.objectives = vec![
                Objective {
                    group: "desk".into(),
                    p95_ms: 30_000,
                },
                Objective {
                    group: "evaluation".into(),
                    p95_ms: 100,
                },
            ];
        })
        .unwrap();
    for _ in 0..3 {
        get(&alice, "/api/me").send().await.unwrap();
    }
    let metrics = metrics_of(get(&alice, "/api/metrics").send().await.unwrap()).await;
    let states: Vec<(&str, u32, ObjectiveState)> = metrics
        .objectives
        .iter()
        .map(|o| (o.group.as_str(), o.p95_ms, o.state))
        .collect();
    assert_eq!(
        states,
        [
            ("desk", 30_000, ObjectiveState::Met),
            ("evaluation", 100, ObjectiveState::Unknown)
        ]
    );
    assert!(
        metrics.objectives[0].measured_p95_ms.is_some()
            && metrics.objectives[1].measured_p95_ms.is_none()
    );
    // Over a long range the hours are left out and the objectives are still said.
    let month = metrics_of(get(&alice, "/api/metrics?days=30").send().await.unwrap()).await;
    assert_eq!(month.objectives.len(), 2);
    // An answer narrowed to a kind or a name is not the whole of a group: it says no objectives.
    for narrowed in ["/api/metrics?kind=serve", "/api/metrics?name=GET%20/api/me"] {
        assert!(
            metrics_of(get(&alice, narrowed).send().await.unwrap())
                .await
                .objectives
                .is_empty(),
            "{narrowed}"
        );
    }
}

#[tokio::test]
async fn an_objective_that_is_not_one_stops_the_node_and_names_the_setting() {
    let dir = tempfile::tempdir().unwrap();
    let data = agency_adapters::files::DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = agency_core::config::Account {
        id: common::id("alice"),
        label: "ALICE".into(),
    };
    for (objective, wanted) in [
        (
            Objective {
                group: "nothing".into(),
                p95_ms: 100,
            },
            "metrics.objectives[0].group: \"nothing\" is not a group of routes",
        ),
        (
            Objective {
                group: "desk".into(),
                p95_ms: 0,
            },
            "metrics.objectives[0].p95Ms: 0 must be 1 to 30000",
        ),
    ] {
        let mut config = NodeConfig::new(
            "http://127.0.0.1:1",
            "127.0.0.1:1",
            account.clone(),
            "scrum",
        );
        config.metrics.objectives = vec![objective];
        data.save_config(&config).unwrap();
        let error = match agency_node::open(data.clone(), agency_node::Overrides::default()) {
            Ok(_) => panic!("the node started"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(wanted), "{error}");
    }
}
