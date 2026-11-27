//! `POST /api/telemetry`: what a client observed of its own calls to this node (docs/protocol.md, "Metrics"),
//! kept as `client` series. It is a write like any other, so the guard has already asked for the write token and
//! the owner. A route the node does not name is dropped and counted, because a client could otherwise make
//! the node hold a series for any text.
use super::{json_body, refuse, respond, ClientApi, Shared};
use crate::metrics::{Recorder, SeriesKey};
use agency_core::Error;
use agency_proto::metrics::{valid_status, SeriesKind, CLIENTS};
use agency_proto::telemetry::{
    TelemetryAnswer, TelemetryReport, MOST_REPORT_BYTES, MOST_SAMPLES, MOST_SAMPLE_MS,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use std::sync::Arc;
use std::time::Duration;

/// The routes of the Client API a client may report, written in `testdata/route-templates.json` for the
/// clients to name theirs from. A test holds the file to this table and this table to the router.
pub const REPORTABLE_ROUTES: [&str; 31] = [
    "GET /api/about",
    "GET /api/claim",
    "POST /api/claim",
    "GET /api/claims",
    "POST /api/claims/{id}/confirm",
    "POST /api/claims/{id}/reject",
    "GET /api/me",
    "GET /api/session",
    "GET /api/decisions",
    "GET /api/devices",
    "POST /api/devices/requests",
    "POST /api/devices/changes",
    "GET /api/metrics",
    "GET /api/orgs/{org}",
    "GET /api/orgs/{org}/effort",
    "GET /api/orgs/{org}/evaluation",
    "GET /api/orgs/{org}/outcomes",
    "GET /api/orgs/{org}/outcomes/observations",
    "POST /api/orgs/{org}/outcomes/observations",
    "GET /api/orgs/{org}/outcomes/readings",
    "GET /api/orgs/{org}/teams/{team}/activity",
    "POST /api/orgs/{org}/teams/{team}/envelopes",
    "GET /api/orgs/{org}/teams/{team}/usage",
    "GET /api/orgs/{org}/teams/{team}/view",
    "GET /api/push/subscriptions",
    "POST /api/push/subscriptions",
    "DELETE /api/push/subscriptions/{id}",
    "GET /api/push/tokens",
    "POST /api/push/tokens",
    "DELETE /api/push/tokens/{token}",
    "GET /api/push/vapid",
];

pub(super) const ROUTES: &[(&str, &str)] = &[("POST", "/api/telemetry")];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new().route(ROUTES[0].1, post(post_telemetry))
}

async fn post_telemetry(State(api): Shared, bytes: Bytes) -> Response {
    if bytes.len() > MOST_REPORT_BYTES {
        return refuse(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!("a report may be at most {MOST_REPORT_BYTES} bytes"),
        );
    }
    respond(
        json_body::<TelemetryReport>(&bytes, "telemetry report")
            .and_then(|report| record(&api.metrics, &report)),
    )
}

/// Records the samples of a report that name a known route and a status a series may have, and counts the
/// rest. A report that is not well formed (an unknown client, too many samples) is refused whole.
fn record(recorder: &Recorder, report: &TelemetryReport) -> Result<TelemetryAnswer, Error> {
    if !CLIENTS.contains(&report.client.as_str()) {
        return Err(Error::bad_request("client must be web or ios"));
    }
    if report.samples.len() > MOST_SAMPLES {
        return Err(Error::bad_request(format!(
            "a report may hold at most {MOST_SAMPLES} samples"
        )));
    }
    let mut answer = TelemetryAnswer {
        accepted: 0,
        dropped: 0,
    };
    for sample in &report.samples {
        if !REPORTABLE_ROUTES.contains(&sample.route.as_str())
            || !valid_status(SeriesKind::Client, &sample.status)
        {
            answer.dropped += 1;
            continue;
        }
        let key = SeriesKey::new(SeriesKind::Client, &sample.route, &sample.status)
            .with_client(&report.client);
        recorder.record(key, Duration::from_millis(sample.ms.min(MOST_SAMPLE_MS)));
        answer.accepted += 1;
    }
    Ok(answer)
}
