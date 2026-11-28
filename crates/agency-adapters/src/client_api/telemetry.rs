//! `POST /api/telemetry`: what a client observed of its own calls to this node (docs/protocol.md, "Metrics"),
//! kept as `client` series. It is a write like any other, so the guard has already asked for the write token and
//! the owner. A route the node does not name is dropped and counted, because a client could otherwise make
//! the node hold a series for any text.
use super::{json_body, refuse, respond, ClientApi, Route, Shared};
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
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// The routes of the Client API a client may report: those declared `reported`, by `<method> <path>`, sorted.
pub fn reportable_routes() -> Vec<String> {
    let mut routes: Vec<String> = super::declared()
        .filter(|route| route.is_reported())
        .map(Route::template)
        .collect();
    routes.sort_unstable();
    routes
}

/// `testdata/route-templates.json` as the declarations make it: the routes a client may report, which the
/// clients name theirs from. A test fails when the file differs.
pub fn route_templates() -> String {
    let file = serde_json::json!({ "routes": reportable_routes() });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&file).expect("a list of strings")
    )
}

pub(super) const ROUTES: &[Route] = &[Route::post("/api/telemetry")];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new().route(ROUTES[0].path, post(post_telemetry))
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
    static REPORTABLE: OnceLock<Vec<String>> = OnceLock::new();
    let reportable = REPORTABLE.get_or_init(reportable_routes);
    for sample in &report.samples {
        if reportable.binary_search(&sample.route).is_err()
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
