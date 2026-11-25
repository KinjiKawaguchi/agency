//! The Client API (`/api`) and the web client's pages. Every request is admitted first:
//! - Direct local: the TCP peer is loopback, `Host` is `127.0.0.1:<port>` or `localhost:<port>`, and the
//!   request did not come through Cloudflare. The peer check matters because the node may also listen
//!   beyond loopback for federation; `Host` alone is chosen by the sender.
//! - Through Cloudflare or at the public host name: only with a valid Access JWT. Without an Access
//!   setting this is refused (fails closed).
//!
//! Writes also need the page's token in `x-agency-token` and a JSON body. A page on another site cannot
//! send that header without a preflight, which this node never allows (CSRF). Through Access, writes
//! also need the owner's email, because the node signs decisions with the owner's account key.
use crate::access::{AccessVerifier, Identity};
use crate::server::status_of;
use agency_core::client::{
    decision_log, me, org_definition, submit_decision, submit_signed_decision, team_snapshot,
};
use agency_core::devices::{device_list, request_device, submit_device_change};
use agency_core::usage::team_usage;
use agency_core::{Error, Node};
use agency_proto::client::{DecisionInput, DeviceRequestInput, SignedDecisionInput};
use agency_proto::{DeviceChange, Id, Signed};
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

pub const TOKEN_HEADER: &str = "x-agency-token";
const ACCESS_HEADER: &str = "cf-access-jwt-assertion";
const BODY_LIMIT: usize = 64 * 1024;
/// The attribute of `<meta name="agency-token">` that the web client's build leaves for the node to fill in.
const TOKEN_PLACEHOLDER: &str = "content=\"__AGENCY_TOKEN__\"";
const PAGE_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const NOT_BUILT: &str = "<!doctype html><meta charset=\"utf-8\"><title>Agency</title><p>The web client is not built into this node. Run <code>npm run build</code> in <code>web/</code>, then rebuild the node.</p>";

/// The built web client's files, by path relative to its root (`index.html`, `assets/app.js`).
pub trait Assets: Send + Sync {
    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>>;
}

pub struct ClientApi {
    pub node: Arc<Node>,
    /// The write token embedded in the page. Random for each start of the node.
    pub token: String,
    /// The port the node listens on, for the `Host` check.
    pub port: u16,
    pub access: Option<Arc<AccessVerifier>>,
    pub assets: Arc<dyn Assets>,
}

type Shared = State<Arc<ClientApi>>;

/// How a request was let in.
#[derive(Clone, Debug)]
enum Admitted {
    Local,
    Access(Identity),
}

fn refuse(status: StatusCode, message: &str) -> Response {
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

fn header_text(headers: &HeaderMap, name: impl header::AsHeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn is_via_cloudflare(headers: &HeaderMap) -> bool {
    headers.contains_key("cf-ray") || headers.contains_key("cf-connecting-ip")
}

async fn admit(
    api: &ClientApi,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<Admitted, (StatusCode, &'static str)> {
    let host = header_text(headers, header::HOST).unwrap_or("");
    let is_public = api
        .access
        .as_ref()
        .is_some_and(|access| access.config().public_host == host);
    if !is_via_cloudflare(headers) && !is_public {
        let is_local_host =
            host == format!("127.0.0.1:{}", api.port) || host == format!("localhost:{}", api.port);
        return match (peer.ip().is_loopback(), is_local_host) {
            (true, true) => Ok(Admitted::Local),
            (true, false) => Err((StatusCode::MISDIRECTED_REQUEST, "wrong host")),
            (false, _) => Err((StatusCode::FORBIDDEN, "the Client API is local only")),
        };
    }
    let access = api
        .access
        .as_ref()
        .ok_or((StatusCode::FORBIDDEN, "this node has no Access setting"))?;
    match access.verify(header_text(headers, ACCESS_HEADER)).await {
        Ok(identity) => Ok(Admitted::Access(identity)),
        Err(reason) => {
            eprintln!("agency: refused an Access request ({reason})");
            Err((
                StatusCode::FORBIDDEN,
                "not signed in through Cloudflare Access",
            ))
        }
    }
}

/// Constant-time comparison, so the token cannot be guessed byte by byte from response times.
fn is_same_secret(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn check_write(
    api: &ClientApi,
    admitted: &Admitted,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, &'static str)> {
    let has_token =
        header_text(headers, TOKEN_HEADER).is_some_and(|token| is_same_secret(token, &api.token));
    if !has_token {
        return Err((StatusCode::FORBIDDEN, "missing or wrong write token"));
    }
    let is_json = header_text(headers, header::CONTENT_TYPE)
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim() == "application/json");
    if !is_json {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "the body must be application/json",
        ));
    }
    let Admitted::Access(identity) = admitted else {
        return Ok(());
    };
    let owner = api
        .access
        .as_ref()
        .map(|access| access.config().owner_email.as_str());
    let is_owner = identity
        .email
        .as_deref()
        .zip(owner)
        .is_some_and(|(email, owner)| email.eq_ignore_ascii_case(owner));
    if is_owner {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "only the node's owner may decide"))
    }
}

async fn guard(
    State(api): Shared,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let admitted = match admit(&api, peer, request.headers()).await {
        Ok(admitted) => admitted,
        Err((status, message)) => return refuse(status, message),
    };
    let is_write = !matches!(*request.method(), Method::GET | Method::HEAD);
    if is_write {
        if let Err((status, message)) = check_write(&api, &admitted, request.headers()) {
            return refuse(status, message);
        }
    }
    next.run(request).await
}

/// A malformed id in the path is reported like a missing one.
fn parse_id(raw: &str) -> Result<Id, Error> {
    Id::parse(raw).map_err(|_| Error::not_found("not found"))
}

async fn get_me(State(api): Shared) -> Response {
    json_response(StatusCode::OK, &me(&api.node))
}

/// The account's recent decisions. `limit` defaults to 50 and is capped at 200.
async fn get_decisions(
    State(api): Shared,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let limit = match query.get("limit").map(|raw| raw.parse::<usize>()) {
        None => None,
        Some(Ok(limit)) => Some(limit),
        Some(Err(_)) => return refuse(StatusCode::BAD_REQUEST, "limit must be a number"),
    };
    json_response(StatusCode::OK, &decision_log(&api.node, limit))
}

async fn get_org(State(api): Shared, Path(org): Path<String>) -> Response {
    respond(parse_id(&org).and_then(|org| org_definition(&api.node, &org)))
}

async fn get_team(State(api): Shared, Path((org, team)): Path<(String, String)>) -> Response {
    let result = async { team_snapshot(&api.node, &parse_id(&org)?, &parse_id(&team)?).await };
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

async fn post_decision(
    State(api): Shared,
    Path((org, team)): Path<(String, String)>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let input: DecisionInput =
            serde_json::from_slice(&bytes).map_err(|_| Error::bad_request("malformed decision"))?;
        submit_decision(&api.node, &org, &team, input).await
    };
    respond(result.await)
}

/// A JSON body, or 400 naming what it should have been.
fn json_body<T: serde::de::DeserializeOwned>(bytes: &Bytes, what: &str) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| Error::bad_request(format!("malformed {what}")))
}

/// A decision signed by one of the account's devices. The node checks and relays it; it never signs.
async fn post_envelope(
    State(api): Shared,
    Path((org, team)): Path<(String, String)>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let input: SignedDecisionInput = json_body(&bytes, "signed decision")?;
        submit_signed_decision(&api.node, &org, &team, input).await
    };
    respond(result.await)
}

async fn get_devices(State(api): Shared) -> Response {
    respond(device_list(&api.node))
}

/// A new device asks to be added; the answer carries its pairing code.
async fn post_device_request(State(api): Shared, bytes: Bytes) -> Response {
    let result = json_body::<DeviceRequestInput>(&bytes, "device request")
        .and_then(|input| request_device(&api.node, input));
    respond(result)
}

/// A device change signed by one of the account's devices, relayed to the org's root node.
async fn post_device_change(State(api): Shared, bytes: Bytes) -> Response {
    let result = async {
        let signed: Signed<DeviceChange> = json_body(&bytes, "device change")?;
        submit_device_change(&api.node, signed).await
    };
    respond(result.await)
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn page_response(content_type: &str, body: Body) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    for (name, value) in [
        ("content-type", content_type),
        ("content-security-policy", PAGE_POLICY),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
    ] {
        headers.insert(
            name,
            HeaderValue::from_str(value).unwrap_or_else(|_| HeaderValue::from_static("")),
        );
    }
    response
}

/// `index.html` with the write token in place of the built page's placeholder meta tag.
fn index_page(api: &ClientApi) -> Response {
    let Some(index) = api.assets.get("index.html") else {
        return page_response("text/html; charset=utf-8", Body::from(NOT_BUILT));
    };
    let filled = format!("content=\"{}\"", api.token);
    let page = String::from_utf8_lossy(&index).replacen(TOKEN_PLACEHOLDER, &filled, 1);
    page_response("text/html; charset=utf-8", Body::from(page))
}

/// A built file, or the page itself for any other path (the web client's own routes).
async fn page(State(api): Shared, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let is_file =
        !path.is_empty() && path != "index.html" && !path.split('/').any(|part| part == "..");
    match api.assets.get(path).filter(|_| is_file) {
        Some(bytes) => page_response(content_type(path), Body::from(bytes.into_owned())),
        None if path.starts_with("api/") || path == "api" => {
            refuse(StatusCode::NOT_FOUND, "no such route")
        }
        None => index_page(&api),
    }
}

/// The Client API and the pages. Serve it with `into_make_service_with_connect_info::<SocketAddr>()`.
pub fn router(api: Arc<ClientApi>) -> Router {
    Router::new()
        .route("/api/me", get(get_me))
        .route("/api/decisions", get(get_decisions))
        .route("/api/devices", get(get_devices))
        .route("/api/devices/requests", post(post_device_request))
        .route("/api/devices/changes", post(post_device_change))
        .route("/api/orgs/{org}", get(get_org))
        .route("/api/orgs/{org}/teams/{team}/view", get(get_team))
        .route("/api/orgs/{org}/teams/{team}/usage", get(get_usage))
        .route(
            "/api/orgs/{org}/teams/{team}/decisions",
            post(post_decision),
        )
        .route(
            "/api/orgs/{org}/teams/{team}/envelopes",
            post(post_envelope),
        )
        .fallback(page)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(api.clone(), guard))
        .with_state(api)
}
