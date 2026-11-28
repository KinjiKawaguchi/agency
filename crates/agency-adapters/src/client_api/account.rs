//! The account's own reads: the session, what the node is, the account and its orgs, and its decisions.
use super::{caller_of, is_owner_request, json_response, refuse, respond, Admitted, Route, Shared};
use crate::server::status_of;
use agency_core::about;
use agency_core::client::{decision_entry, home_nodes, me};
use agency_core::Error;
use agency_proto::client::Session;
use agency_proto::Timestamp;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;

use super::ClientApi;

pub(super) const ROUTES: &[Route] = &[
    Route::get("/api/about").for_guests().reported(),
    Route::get("/api/me").for_guests().reported(),
    Route::get("/api/session").for_guests().reported(),
    Route::get("/api/decisions").for_guests().reported(),
    Route::get("/api/decisions/{id}"),
];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new()
        .route(ROUTES[0].path, get(get_about))
        .route(ROUTES[1].path, get(get_me))
        .route(ROUTES[2].path, get(get_session))
        .route(ROUTES[3].path, get(get_decisions))
        .route(ROUTES[4].path, get(get_decision))
}

/// The write token and the node's time, for clients that do not read the page (the iOS app). Any reader
/// may have it, as any reader may load the page that carries it; writes through Access also need the
/// owner's sign-in.
async fn get_session(State(api): Shared) -> Response {
    let now =
        Timestamp::parse(&api.node.now_text()).map_err(|error| Error::internal(error.to_string()));
    respond(now.map(|now| Session {
        write_token: api.token.clone(),
        now,
    }))
}

/// What this node is and speaks, so an app can tell whether the node or the app needs an update.
async fn get_about() -> Response {
    json_response(StatusCode::OK, &about::this_node())
}

/// The account and what it reaches; to the node's owner, also their other home nodes, which say where
/// else the owner works and are no one else's to know.
async fn get_me(State(api): Shared, Extension(admitted): Extension<Admitted>) -> Response {
    if let Admitted::Guest(guest) = &admitted {
        return respond(agency_core::guest_reads::me(&api.node, guest));
    }
    let mut answer = me(&api.node);
    if is_owner_request(&api, &admitted) {
        let listed = home_nodes(&api.node.config()).0;
        answer.home_nodes = (!listed.is_empty()).then_some(listed);
        let held = match agency_core::pending_root::waiting(&api.node) {
            Ok(held) => held,
            Err(error) => return refuse(status_of(error.kind), &error.message),
        };
        answer.root_changes =
            (!held.is_empty()).then(|| held.iter().map(|one| one.as_ref_for_desk()).collect());
    }
    json_response(StatusCode::OK, &answer)
}

/// The account's recent decisions. `limit` defaults to 50 and is capped at 200.
async fn get_decisions(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let limit = match query.get("limit").map(|raw| raw.parse::<usize>()) {
        None => None,
        Some(Ok(limit)) => Some(limit),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "limit must be a number"),
    };
    let log = agency_core::client::decision_log_of(&api.node, &caller_of(&admitted), limit);
    json_response(StatusCode::OK, &log)
}

/// One decision of the account with where it is now, which a client that sent it reads until it is applied,
/// delivered, or refused.
async fn get_decision(State(api): Shared, Path(id): Path<String>) -> Response {
    respond(decision_entry(&api.node, &id))
}
