//! Requests to other nodes name the node in `User-Agent`; proxies in front of a node may refuse requests
//! without one.
use agency_adapters::peers::HttpPeers;
use agency_core::ports::Peers;
use agency_core::read_auth::ReadAuth;
use axum::http::HeaderMap;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn requests_to_other_nodes_carry_a_user_agent() {
    let seen: Arc<Mutex<Option<String>>> = Arc::default();
    let record = seen.clone();
    let app = axum::Router::new().route(
        "/fed/v1/teams/{org}/{team}/view",
        axum::routing::get(move |headers: HeaderMap| async move {
            *record.lock().unwrap() = headers
                .get("user-agent")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            axum::http::StatusCode::NOT_FOUND
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let peers = HttpPeers::new().unwrap();
    let _ = peers
        .get_view(&url, "/fed/v1/teams/acme/app/view", &ReadAuth::default())
        .await;
    let agent = seen.lock().unwrap().clone().unwrap_or_default();
    assert!(agent.starts_with("agency-node/"), "{agent:?}");
}
