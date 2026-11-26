//! Measuring the requests a node answers: one middleware for every route, the federation's and the Client
//! API's. A request is named by the route template it matched (`GET /api/orgs/{org}/teams/{team}/usage`),
//! never by its path, so ids are not in a name; one that matched no route (the web client's files, every 404,
//! every scanner) is the one series `unmatched`; and a method outside the standard set is `OTHER`, because a
//! requester picks it.

use super::{Recorder, SeriesKey};
use agency_proto::metrics::{SeriesKind, UNMATCHED};
use axum::extract::{MatchedPath, Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

/// The method as a word of a name: the standard ones, and `OTHER` for any other.
fn method_word(method: &Method) -> &'static str {
    [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::HEAD,
        Method::OPTIONS,
    ]
    .iter()
    .find(|standard| *standard == method)
    .map_or("OTHER", Method::as_str)
}

/// The class of a status. An informational one (which a handler does not send) is counted as a success.
fn status_class(status: StatusCode) -> &'static str {
    match status.as_u16() / 100 {
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        _ => "2xx",
    }
}

/// The name of a request's series from its method and what it matched. A request that a route refused for
/// its method (a 405) is named with `OTHER`, not with the method it asked with: the route is the node's, but
/// the method is the requester's, and a series for each would be a way to make many.
fn series_name(method: &Method, matched: Option<&str>, status: StatusCode) -> String {
    let word = if status == StatusCode::METHOD_NOT_ALLOWED {
        "OTHER"
    } else {
        method_word(method)
    };
    match matched {
        Some(template) => format!("{word} {template}"),
        None => UNMATCHED.to_owned(),
    }
}

/// Times the rest of the stack and records how it went.
pub async fn record_request(
    State(recorder): State<Recorder>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let matched = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str().to_owned());
    let started = Instant::now();
    let response = next.run(request).await;
    let key = SeriesKey::new(
        SeriesKind::Serve,
        series_name(&method, matched.as_deref(), response.status()),
        status_class(response.status()),
    );
    recorder.record(key, started.elapsed());
    response
}

#[cfg(test)]
mod tests;
