//! A node does not follow redirects from other nodes: others choose the origins it sends to, and a
//! redirect would send its signed requests somewhere else.
mod common;

use agency_core::orgs;
use agency_core::views::refresh;
use agency_proto::org::TeamPlace;
use common::{alice, id};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn a_redirect_from_another_node_is_not_followed() {
    let (alice, _) = alice().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_url = format!("http://{}/anywhere", target.local_addr().unwrap());
    let elsewhere = axum::Router::new().fallback(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        async { "{}" }
    });
    tokio::spawn(async move { axum::serve(target, elsewhere).await.unwrap() });

    let far = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let far_url = format!("http://{}", far.local_addr().unwrap());
    let redirecting = axum::Router::new().fallback(move || {
        let location = target_url.clone();
        async move { axum::response::Redirect::temporary(&location) }
    });
    tokio::spawn(async move { axum::serve(far, redirecting).await.unwrap() });

    let place = TeamPlace {
        id: id("far"),
        label: "Far".into(),
        department: None,
        node: far_url,
        plan: None,
    };
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();
    refresh(&alice.node).await.unwrap();
    let replica = alice
        .node
        .store()
        .load_replica(&id("acme"), &id("far"))
        .unwrap();
    assert!(
        replica
            .error
            .as_deref()
            .is_some_and(|error| error.contains("307")),
        "{:?}",
        replica.error
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "the redirect's target was never asked"
    );
}
