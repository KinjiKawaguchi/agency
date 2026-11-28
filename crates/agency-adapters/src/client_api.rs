//! The Client API (`/api`) and the web client's pages. Every request is admitted first:
//! - Direct local: the TCP peer is loopback, `Host` is `127.0.0.1:<port>` or `localhost:<port>`, and the
//!   request did not come through Cloudflare. The peer check matters because the node may also listen
//!   beyond loopback for federation; `Host` alone is chosen by the sender.
//! - Through Cloudflare or at the public host name: only with a valid Access JWT. Without an Access
//!   setting this is refused (fails closed).
//!
//! Writes also need the page's token in `x-agency-token` and a JSON body. A page on another site cannot
//! send that header without a preflight, which this node never allows (CSRF). Through Access, writes
//! also need the owner's email, because writes act for the node's account.
use crate::access::{Identity, SignInVerifier};
use crate::handoff::{self, ASSOCIATION_PATH, CALLBACK_PATH, HANDOFF_PATH, REDEEM_PATH};
use crate::launch::Launch;
use crate::server::status_of;
use agency_core::guests::Guest;
use agency_core::{Error, Node};
use agency_proto::client_version::Platform;
use agency_proto::clock::OffsetDateTime;
use agency_proto::Id;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod account;
mod admission;
mod claimants;
mod claims;
mod devices;
mod guests;
mod history;
mod members;
mod metrics;
mod orgs;
mod outcomes;
mod pages;
mod push;
mod route;
mod telemetry;

pub use admission::is_via_cloudflare;
use admission::{caller_of, guard, is_owner_request};
pub(crate) use admission::{is_owner, is_same_secret};
pub(crate) use pages::page_response;
pub use route::Route;
pub use telemetry::{reportable_routes, route_templates};

pub const TOKEN_HEADER: &str = "x-agency-token";
const BODY_LIMIT: usize = 64 * 1024;

/// The built web client's files, by path relative to its root (`index.html`, `assets/app.js`).
pub trait Assets: Send + Sync {
    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>>;
}

pub struct ClientApi {
    pub node: Arc<Node>,
    /// Where the Client API writes a line for the node's log.
    pub log: crate::Log,
    /// The node's own measurements, which the owner reads.
    pub metrics: crate::metrics::Recorder,
    /// The write token embedded in the page. Random for each start of the node.
    pub token: String,
    /// The port the node listens on, for the `Host` check.
    pub port: u16,
    /// The sign-in provider in front of the Client API (Access or OIDC), when one is set and well formed.
    pub access: Option<Arc<SignInVerifier>>,
    pub assets: Arc<dyn Assets>,
    /// The running program and its data directory, for the command a client shows the owner. `None`
    /// when they cannot be written out as text; clients then show their own wording.
    pub launch: Option<Launch>,
    /// When each app version was last noted, so a client that calls often costs no write (see `note_client`).
    pub client_notes: Mutex<HashMap<(Platform, String), OffsetDateTime>>,
    /// The codes the app handoff made and that wait to be redeemed.
    pub handoff: handoff::Codes,
    /// The tries of each client address to redeem a claim link's code.
    pub claim_attempts: Mutex<crate::attempts::Attempts>,
}

pub(crate) type Shared = State<Arc<ClientApi>>;

/// How a request was let in.
#[derive(Clone, Debug)]
pub(crate) enum Admitted {
    Local,
    Access(Identity),
    /// A member this node hosts, signed in through Access with the email its record names. It reaches only
    /// the routes declared `for_guests`, scoped to its member.
    Guest(Guest),
    /// A person signed in through Access with the address of a claim link, who is not yet a guest: the address
    /// (lowercased) is all there is of them. It reaches only the claim routes (declared `for_claimants`) and the
    /// web client's pages, and acts as no one.
    Claimant(String),
}

pub(crate) fn refuse(status: StatusCode, message: &str) -> Response {
    json_response(status, &json!({ "error": message }))
}

fn json_response<T: Serialize>(status: StatusCode, body: &T) -> Response {
    let headers = [
        (header::CACHE_CONTROL, "no-store"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ];
    (status, headers, Json(body)).into_response()
}

fn respond<T: Serialize>(result: Result<T, Error>) -> Response {
    match result {
        Ok(body) => json_response(StatusCode::OK, &body),
        Err(error) => refuse(status_of(error.kind), &error.message),
    }
}

pub(crate) fn header_text(headers: &HeaderMap, name: impl header::AsHeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// A malformed id in the path is reported like a missing one.
fn parse_id(raw: &str) -> Result<Id, Error> {
    Id::parse(raw).map_err(|_| Error::not_found("not found"))
}

/// A JSON body, or 400 naming what it should have been.
fn json_body<T: serde::de::DeserializeOwned>(bytes: &Bytes, what: &str) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| Error::bad_request(format!("malformed {what}")))
}

/// The routes behind the guard that this module serves itself: the app's sign-in, whose handlers are in
/// `handoff`.
const ROUTES: &[Route] = &[Route::get(HANDOFF_PATH), Route::get(CALLBACK_PATH)];

/// Every part's routes, this module's first: a new part of the Client API adds its table here.
const PARTS: [&[Route]; 12] = [
    ROUTES,
    account::ROUTES,
    claimants::ROUTES,
    push::ROUTES,
    devices::ROUTES,
    orgs::ROUTES,
    history::ROUTES,
    outcomes::ROUTES,
    members::ROUTES,
    metrics::ROUTES,
    telemetry::ROUTES,
    claims::ROUTES,
];

/// Every route behind the guard, as declared.
pub(super) fn declared() -> impl Iterator<Item = &'static Route> {
    PARTS.into_iter().flatten()
}

/// The routes a claimant may reach, by method and path, for the tests.
pub fn claimant_routes() -> Vec<(&'static str, &'static str)> {
    declared()
        .filter(|route| route.is_for_claimants())
        .map(Route::pair)
        .collect()
}

/// The routes a guest may reach, by method and path, for the tests.
pub fn guest_routes() -> Vec<(&'static str, &'static str)> {
    declared()
        .filter(|route| route.is_for_guests())
        .map(Route::pair)
        .collect()
}

/// Every route behind the guard, this module's and its parts', by method and path.
pub fn routes_behind_the_guard() -> Vec<(&'static str, &'static str)> {
    declared().map(Route::pair).collect()
}

/// The Client API and the pages. Serve it with `into_make_service_with_connect_info::<SocketAddr>()`.
pub fn router(api: Arc<ClientApi>) -> Router {
    Router::new()
        .route(HANDOFF_PATH, get(handoff::handoff))
        .route(CALLBACK_PATH, get(handoff::callback))
        .merge(account::routes())
        .merge(claimants::routes())
        .merge(push::routes())
        .merge(devices::routes())
        .merge(orgs::routes())
        .merge(history::routes(api.clone()))
        .merge(outcomes::routes(api.clone()))
        .merge(members::routes())
        .merge(metrics::routes(api.clone()))
        .merge(telemetry::routes())
        .merge(claims::routes())
        .fallback(pages::page)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(api.clone(), guard))
        // Added after the guard, so it answers without an Access sign-in: Apple's servers fetch it.
        .route(ASSOCIATION_PATH, get(handoff::association))
        .route(REDEEM_PATH, post(handoff::redeem))
        .with_state(api)
}
