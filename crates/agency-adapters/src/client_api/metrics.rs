//! The routes of the node's own measurements (docs/protocol.md, "Metrics"). They are the node's data and
//! the owner's alone: the owner is the direct local request, or the Access sign-in of the configured owner,
//! and nobody else the node lets in. They are not federation routes, so no other node reads them.
use super::{is_owner_request, refuse, respond, Admitted, ClientApi, Shared};
use agency_core::Error;
use agency_proto::metrics::{valid_metrics_days, SeriesKind, MAX_METRICS_DAYS};
use axum::extract::{Extension, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;

/// The range a read covers when it does not say.
pub const DEFAULT_DAYS: u32 = 1;
/// The longest range an answer carries the hours of: a longer one is the series' totals only, so that an
/// answer stays small (a node holds a series for each route and status, and 30 days are 720 hours).
pub const HOURS_UP_TO_DAYS: u32 = 7;

async fn require_owner(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    request: Request,
    next: Next,
) -> Response {
    if !is_owner_request(&api, &admitted) {
        return refuse(
            StatusCode::FORBIDDEN,
            "only the node's owner may read its measurements",
        );
    }
    next.run(request).await
}

pub(super) fn routes(api: Arc<ClientApi>) -> Router<Arc<ClientApi>> {
    let owner_only = Router::new()
        .route("/api/metrics", get(get_metrics))
        .route_layer(middleware::from_fn_with_state(api, require_owner));
    owner_only.route("/metrics", get(get_prometheus))
}

/// `GET /api/metrics?days=<1 to 30>&kind=&name=`: the measurements of the last `days` days (1 unless it says),
/// of one kind and one name if asked, with the hours of a range of up to 7 days.
async fn get_metrics(State(api): Shared, Query(query): Query<HashMap<String, String>>) -> Response {
    let days = match query.get("days").map(|raw| raw.parse::<u32>()) {
        None => DEFAULT_DAYS,
        Some(Ok(days)) if valid_metrics_days(days) => days,
        Some(_) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                &format!("days must be a number from 1 to {MAX_METRICS_DAYS}"),
            )
        }
    };
    let kind = match query.get("kind").map(String::as_str) {
        None => None,
        Some("serve") => Some(SeriesKind::Serve),
        Some("call") => Some(SeriesKind::Call),
        Some("client") => Some(SeriesKind::Client),
        Some(_) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                "kind must be serve, call, or client",
            )
        }
    };
    let mut metrics = api
        .metrics
        .read(days, kind, query.get("name").map(String::as_str));
    if days > HOURS_UP_TO_DAYS {
        for series in &mut metrics.series {
            series.by_hour.clear();
        }
    }
    respond(Ok::<_, Error>(metrics))
}

/// `GET /metrics`: the measurements since the node started, as the text a Prometheus scrape reads. Only a
/// direct request from this machine: not one through Cloudflare, whoever is signed in.
async fn get_prometheus(State(api): Shared, Extension(admitted): Extension<Admitted>) -> Response {
    if !matches!(admitted, Admitted::Local) {
        return refuse(
            StatusCode::FORBIDDEN,
            "the measurements are scraped on the node's own machine",
        );
    }
    let text = crate::metrics::prometheus::text(&api.metrics.totals());
    let headers = [
        (
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        ),
        (header::CACHE_CONTROL, "no-store"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ];
    (StatusCode::OK, headers, text).into_response()
}
