//! The routes of the outcome analysis (docs/protocol.md, "Outcomes"). Business data: every route is owner-only,
//! a read as well as the write, unlike the rest of the Client API, because someone who may read a team's board
//! must not read the revenue.
use super::{is_owner_request, json_body, refuse, respond, Admitted, ClientApi, Shared};
use agency_core::outcomes;
use agency_proto::observation::ObservationInput;
use agency_proto::Id;
use axum::body::Bytes;
use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;

/// Lets the owner through and refuses everyone else, for every route here: one place for the rule, whichever
/// route and whichever method. A write also meets `check_write`, which asks the same of a sign-in through
/// Access; a read meets only this, so this is the line that keeps the numbers from the other people Access
/// lets in.
async fn require_owner(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    request: Request,
    next: Next,
) -> Response {
    if !is_owner_request(&api, &admitted) {
        return refuse(
            StatusCode::FORBIDDEN,
            "only the node's owner may read or enter the org's numbers",
        );
    }
    next.run(request).await
}

/// The three routes of the numbers, every one behind `require_owner`.
pub(super) fn routes(api: Arc<ClientApi>) -> Router<Arc<ClientApi>> {
    Router::new()
        .route("/api/orgs/{org}/outcomes", get(get_outcomes))
        .route(
            "/api/orgs/{org}/outcomes/observations",
            get(get_history).post(post_observation),
        )
        .route_layer(middleware::from_fn_with_state(api, require_owner))
}

fn org_of(raw: &str) -> Result<Id, agency_core::Error> {
    Id::parse(raw).map_err(|_| agency_core::Error::not_found("not found"))
}

async fn get_outcomes(
    api: Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let days = match query.get("days").map(|raw| raw.parse::<u32>()) {
        None => None,
        Some(Ok(days)) => Some(days),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "days must be a number"),
    };
    respond(org_of(&org).and_then(|org| outcomes::outcomes(&api.node, &org, days)))
}

async fn get_history(
    api: Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let Some(kpi) = query.get("kpi").and_then(|raw| Id::parse(raw).ok()) else {
        return refuse(StatusCode::BAD_REQUEST, "kpi must be the id of a KPI");
    };
    let limit = match query.get("limit").map(|raw| raw.parse::<u32>()) {
        None => None,
        Some(Ok(limit)) => Some(limit),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "limit must be a number"),
    };
    respond(org_of(&org).and_then(|org| outcomes::history(&api.node, &org, &kpi, limit)))
}

async fn post_observation(api: Shared, Path(org): Path<String>, bytes: Bytes) -> Response {
    let result = async {
        let org = org_of(&org)?;
        let input: ObservationInput = json_body(&bytes, "observation")?;
        outcomes::record(&api.node, &org, input).await
    };
    respond(result.await)
}
