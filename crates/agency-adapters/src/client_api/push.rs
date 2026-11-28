//! Where the node sends its notices: the APNs tokens and the browsers' Web Push subscriptions of the owner
//! or of a guest, and the node's VAPID key a browser subscribes with.
use super::{
    caller_of, json_body, json_response, refuse, respond, Admitted, ClientApi, Route, Shared,
};
use crate::server::status_of;
use agency_core::push::{push_tokens_of, register_push_token_as, remove_push_token_as};
use agency_core::web_push::{
    register_subscription_as, remove_subscription_as, subscriptions_of, vapid_key,
};
use agency_core::Error;
use agency_proto::client::{PushSubscriptionInput, PushTokenInput};
use axum::body::Bytes;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::Router;
use std::sync::Arc;

const TOKENS: &str = "/api/push/tokens";
const SUBSCRIPTIONS: &str = "/api/push/subscriptions";

pub(super) const ROUTES: &[Route] = &[
    Route::get(TOKENS).for_guests().reported(),
    Route::post(TOKENS).for_guests().reported(),
    Route::delete("/api/push/tokens/{token}")
        .for_guests()
        .reported(),
    Route::get("/api/push/vapid").for_guests().reported(),
    Route::get(SUBSCRIPTIONS).for_guests().reported(),
    Route::post(SUBSCRIPTIONS).for_guests().reported(),
    Route::delete("/api/push/subscriptions/{id}")
        .for_guests()
        .reported(),
];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new()
        .route(TOKENS, get(get_push_tokens).post(post_push_token))
        .route(ROUTES[2].path, delete(delete_push_token))
        .route(ROUTES[3].path, get(get_vapid_key))
        .route(
            SUBSCRIPTIONS,
            get(get_push_subscriptions).post(post_push_subscription),
        )
        .route(ROUTES[6].path, delete(delete_push_subscription))
}

async fn get_push_tokens(State(api): Shared, Extension(admitted): Extension<Admitted>) -> Response {
    json_response(
        StatusCode::OK,
        &push_tokens_of(&api.node, &caller_of(&admitted)),
    )
}

/// Registers an APNs device token for the owner's notifications (or updates its environment and label).
async fn post_push_token(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    bytes: Bytes,
) -> Response {
    let caller = caller_of(&admitted);
    let result = json_body::<PushTokenInput>(&bytes, "push token")
        .and_then(|input| register_push_token_as(&api.node, &caller, input));
    respond(result)
}

async fn delete_push_token(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    Path(token): Path<String>,
) -> Response {
    match remove_push_token_as(&api.node, &caller_of(&admitted), &token) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => refuse(status_of(error.kind), &error.message),
    }
}

/// The node's VAPID public key, which a browser subscribes with. 404 when the node has no Web Push.
async fn get_vapid_key(State(api): Shared) -> Response {
    respond(vapid_key(&api.node).ok_or_else(|| Error::not_found("this node has no Web Push")))
}

async fn get_push_subscriptions(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
) -> Response {
    respond(subscriptions_of(&api.node, &caller_of(&admitted)))
}

/// Registers a browser's Web Push subscription for the owner's notifications (or updates it).
async fn post_push_subscription(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    bytes: Bytes,
) -> Response {
    let caller = caller_of(&admitted);
    let result = json_body::<PushSubscriptionInput>(&bytes, "push subscription")
        .and_then(|input| register_subscription_as(&api.node, &caller, input));
    respond(result)
}

async fn delete_push_subscription(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    Path(id): Path<String>,
) -> Response {
    match remove_subscription_as(&api.node, &caller_of(&admitted), &id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => refuse(status_of(error.kind), &error.message),
    }
}
