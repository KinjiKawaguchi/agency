//! An org's and its teams' reads, and the decisions a client sends to a team.
use super::{
    caller_of, json_body, json_response, parse_id, refuse, respond, Admitted, ClientApi, Route,
    Shared,
};
use crate::server::status_of;
use agency_core::activity::team_activity;
use agency_core::client::{org_definition, submit_signed_decision, team_snapshot};
use agency_core::effort::org_effort;
use agency_core::evaluation::org_evaluation;
use agency_core::usage::team_usage;
use agency_core::{Error, ErrorKind};
use agency_proto::client::{DecisionOutcome, SignedDecisionInput};
use agency_proto::Timestamp;
use axum::body::Bytes;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) const ROUTES: &[Route] = &[
    Route::get("/api/orgs/{org}").reported(),
    Route::get("/api/orgs/{org}/teams/{team}/view")
        .for_guests()
        .reported(),
    Route::get("/api/orgs/{org}/teams/{team}/usage").reported(),
    Route::get("/api/orgs/{org}/teams/{team}/budget").reported(),
    Route::get("/api/orgs/{org}/evaluation").reported(),
    Route::get("/api/orgs/{org}/effort").reported(),
    Route::get("/api/orgs/{org}/teams/{team}/activity").reported(),
    Route::post("/api/orgs/{org}/teams/{team}/envelopes")
        .for_guests()
        .reported(),
];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new()
        .route(ROUTES[0].path, get(get_org))
        .route(ROUTES[1].path, get(get_team))
        .route(ROUTES[2].path, get(get_usage))
        .route(ROUTES[3].path, get(get_budget))
        .route(ROUTES[4].path, get(get_evaluation))
        .route(ROUTES[5].path, get(get_effort))
        .route(ROUTES[6].path, get(get_activity))
        .route(ROUTES[7].path, post(post_envelope))
}

async fn get_org(State(api): Shared, Path(org): Path<String>) -> Response {
    respond(parse_id(&org).and_then(|org| org_definition(&api.node, &org)))
}

async fn get_team(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    Path((org, team)): Path<(String, String)>,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        match &admitted {
            Admitted::Guest(guest) => {
                agency_core::guest_reads::team_snapshot(&api.node, guest, &org, &team).await
            }
            Admitted::Local | Admitted::Access(_) => team_snapshot(&api.node, &org, &team).await,
            Admitted::Claimant(_) => Err(Error::new(
                agency_core::ErrorKind::Forbidden,
                "a claimant may not do this",
            )),
        }
    };
    respond(result.await)
}

/// A team's token usage over the last `days` days (default 14, at most 90).
async fn get_usage(
    State(api): Shared,
    Path((org, team)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let days = match query.get("days").map(|raw| raw.parse::<u32>()) {
        None => None,
        Some(Ok(days)) => Some(days),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "days must be a number"),
    };
    let result = async { team_usage(&api.node, &parse_id(&org)?, &parse_id(&team)?, days).await };
    respond(result.await)
}

/// A team's weekly budget status: this week's spend of the team and of the org against their caps.
async fn get_budget(State(api): Shared, Path((org, team)): Path<(String, String)>) -> Response {
    let result = async {
        crate::budget::budget_status(&api.node, &parse_id(&org)?, &parse_id(&team)?).await
    };
    respond(result.await)
}

/// The delivery evaluation of every team of an org over the last `days` days (default 14, at most 90).
async fn get_evaluation(
    State(api): Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let days = match query.get("days").map(|raw| raw.parse::<u32>()) {
        None => None,
        Some(Ok(days)) => Some(days),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "days must be a number"),
    };
    let result = async { org_evaluation(&api.node, &parse_id(&org)?, days).await };
    respond(result.await)
}

/// Where the tokens of every team of an org went over the last `days` days, by work type (default 14,
/// at most 90).
async fn get_effort(
    State(api): Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let days = match query.get("days").map(|raw| raw.parse::<u32>()) {
        None => None,
        Some(Ok(days)) => Some(days),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "days must be a number"),
    };
    let result = async { org_effort(&api.node, &parse_id(&org)?, days).await };
    respond(result.await)
}

/// A team's activity: the events after `since` (RFC 3339 UTC, default an hour ago), at most `limit`
/// (default 200, at most 500), oldest first.
async fn get_activity(
    State(api): Shared,
    Path((org, team)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let since = match query.get("since").map(|raw| Timestamp::parse(raw)) {
        None => None,
        Some(Ok(since)) => Some(since),
        Some(Err(_)) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                "since must be an RFC 3339 UTC time",
            )
        }
    };
    let limit = match query.get("limit").map(|raw| raw.parse::<u32>()) {
        None => None,
        Some(Ok(limit)) => Some(limit),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "limit must be a number"),
    };
    let result =
        async { team_activity(&api.node, &parse_id(&org)?, &parse_id(&team)?, since, limit).await };
    respond(result.await)
}

/// A decision signed by one of the account's devices. The node checks and relays it; it never signs.
async fn post_envelope(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    Path((org, team)): Path<(String, String)>,
    bytes: Bytes,
) -> Response {
    let result = (|| {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let input: SignedDecisionInput = json_body(&bytes, "signed decision")?;
        submit_signed_decision(&api.node, &caller_of(&admitted), &org, &team, input)
    })();
    match result {
        Ok(result) if result.outcome == DecisionOutcome::Queued => {
            json_response(StatusCode::ACCEPTED, &result)
        }
        Ok(result) => json_response(StatusCode::OK, &result),
        Err(error) if error.kind == ErrorKind::Unavailable => {
            let mut response = refuse(StatusCode::SERVICE_UNAVAILABLE, &error.message);
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_static(KEEP_RETRY_AFTER_SECONDS),
            );
            response
        }
        Err(error) => refuse(status_of(error.kind), &error.message),
    }
}

/// What a client is told to wait before it sends a decision again that this node could not keep.
const KEEP_RETRY_AFTER_SECONDS: &str = "5";
