//! `agency serve`: the node running, with the federation routes, the Client API and the web client on one
//! port, and its background tasks.
use crate::status_cli::{warn_misplaced, warn_unknown_addressees};
use agency_core::logging_config::LogLevel;
use anyhow::{Context, Result};
use std::time::Duration;

pub async fn serve(opened: agency_node::Opened) -> Result<()> {
    let node = opened.node.clone();
    let listen = node.config().listen;
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .with_context(|| format!("cannot listen on {listen}"))?;
    let port = listener.local_addr()?.port();
    let app = opened.app(port)?;
    println!("Node running: {} (listening on {listen})", node.url());
    let logger = opened.logger().clone();
    let recorder = opened.recorder().clone();
    let flushing = recorder.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let flushing = flushing.clone();
            let _ = tokio::task::spawn_blocking(move || flushing.flush()).await;
        }
    });
    logger.record(
        LogLevel::Info,
        "start",
        &format!("node running: {} (listening on {listen})", node.url()),
    );
    warn_misplaced(&node, |line| logger.warn("placement", &line));
    warn_unknown_addressees(&node, |line| logger.warn("placement", &line));
    for line in agency_core::client::home_nodes(&node.config()).1 {
        logger.warn("placement", &line);
    }
    println!("Web client: http://127.0.0.1:{port}/");
    opened.start_usage_collector();
    opened.start_delivery_connectors();
    agency_node::background::start(
        node.clone(),
        logger.clone(),
        agency_node::background::Schedule::default(),
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    let _ = tokio::task::spawn_blocking(move || recorder.flush()).await;
    Ok(())
}
