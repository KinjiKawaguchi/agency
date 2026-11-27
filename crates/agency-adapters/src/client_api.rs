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
use crate::access::{AccessVerifier, Identity};
use crate::handoff::{
    self, ACCESS_HEADER, ASSOCIATION_PATH, CALLBACK_PATH, HANDOFF_PATH, REDEEM_PATH,
};
use crate::launch::Launch;
use crate::server::status_of;
use agency_core::about;
use agency_core::activity::team_activity;
use agency_core::client::{home_nodes, me, org_definition, submit_signed_decision, team_snapshot};
use agency_core::clients_seen;
use agency_core::devices::{device_list, request_device};
use agency_core::effort::org_effort;
use agency_core::evaluation::org_evaluation;
use agency_core::guests::{Caller, Guest};
use agency_core::push::{push_tokens_of, register_push_token_as, remove_push_token_as};
use agency_core::usage::team_usage;
use agency_core::web_push::{
    register_subscription_as, remove_subscription_as, subscriptions_of, vapid_key,
};
use agency_core::{Error, Node};
use agency_proto::client::{
    DeviceList, DeviceRequestInput, NodeInfo, PushSubscriptionInput, PushTokenInput, Session,
    SignedDecisionInput,
};
use agency_proto::client_version::{self, Platform};
use agency_proto::clock::{Duration, OffsetDateTime};
use agency_proto::{DeviceChange, Id, Signed, Timestamp};
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Extension, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, MethodRouter};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

mod guests;
mod metrics;
mod outcomes;
mod telemetry;

pub use telemetry::REPORTABLE_ROUTES;

pub const TOKEN_HEADER: &str = "x-agency-token";
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
    /// Where the Client API writes a line for the node's log.
    pub log: crate::Log,
    /// The node's own measurements, which the owner reads.
    pub metrics: crate::metrics::Recorder,
    /// The write token embedded in the page. Random for each start of the node.
    pub token: String,
    /// The port the node listens on, for the `Host` check.
    pub port: u16,
    pub access: Option<Arc<AccessVerifier>>,
    pub assets: Arc<dyn Assets>,
    /// The running program and its data directory, for the command a client shows the owner. `None`
    /// when they cannot be written out as text; clients then show their own wording.
    pub launch: Option<Launch>,
    /// When each app version was last noted, so a client that calls often costs no write (see `note_client`).
    pub client_notes: Mutex<HashMap<(Platform, String), OffsetDateTime>>,
    /// The codes the app handoff made and that wait to be redeemed.
    pub handoff: handoff::Codes,
}

pub(crate) type Shared = State<Arc<ClientApi>>;

/// How a request was let in.
#[derive(Clone, Debug)]
pub(crate) enum Admitted {
    Local,
    Access(Identity),
    /// A member this node hosts, signed in through Access with the email its record names. It reaches only
    /// the routes in `guests::GUEST_ROUTES`, scoped to its member.
    Guest(Guest),
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

/// Whether the request came through Cloudflare (its tunnel or proxy adds these headers).
pub fn is_via_cloudflare(headers: &HeaderMap) -> bool {
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
            // On a machine other accounts log in to, each of them reaches the loopback address too.
            (true, true) if !api.node.config().local_clients => {
                Err((StatusCode::FORBIDDEN, "this node admits no local clients"))
            }
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
        // Only a known identity is admitted: the owner's email, which always wins, or a current guest's.
        // Any other sign-in, one without an email included, is refused.
        Ok(identity) if is_owner(api, &identity) => Ok(Admitted::Access(identity)),
        Ok(identity) => identity
            .email
            .as_deref()
            .and_then(|email| agency_core::guests::admit(&api.node, email))
            .map(Admitted::Guest)
            .ok_or_else(|| {
                (api.log)("refused an Access sign-in that is neither the owner nor a guest");
                (
                    StatusCode::FORBIDDEN,
                    "this sign-in is not the node's owner or a guest of it",
                )
            }),
        Err(reason) => {
            (api.log)(&format!("refused an Access request ({reason})"));
            Err((
                StatusCode::FORBIDDEN,
                "not signed in through Cloudflare Access",
            ))
        }
    }
}

/// Constant-time comparison, so the token cannot be guessed byte by byte from response times.
pub(crate) fn is_same_secret(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// A write needs the page's token, a JSON body (a DELETE has none), and the owner: on a node with an
/// access setting, the owner signed in through Access, even on loopback.
fn check_write(
    api: &ClientApi,
    admitted: &Admitted,
    method: &Method,
    path: &str,
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
    if !is_json && method != Method::DELETE {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "the body must be application/json",
        ));
    }
    match admitted {
        // With an access setting the owner is who Access says it is, so a process of the same OS user
        // on this machine cannot write as the owner. Checked against the setting itself, so a malformed
        // one (which disables Access) does not make loopback the owner again.
        Admitted::Local if api.node.config().access.is_some() => Err((
            StatusCode::FORBIDDEN,
            "this node is behind Access: writes come through its public host name, signed in as the owner",
        )),
        Admitted::Local => Ok(()),
        Admitted::Access(identity) if is_owner(api, identity) => Ok(()),
        Admitted::Access(_) => Err((StatusCode::FORBIDDEN, "only the node's owner may decide")),
        // A guest never writes as the owner: only the writes of its own table, scoped to its member there.
        Admitted::Guest(guest) if guests::may_write(guest, method, path) => Ok(()),
        Admitted::Guest(_) => Err((StatusCode::FORBIDDEN, "a guest may not do this")),
    }
}

/// Whether the Access sign-in is the configured owner's.
pub(crate) fn is_owner(api: &ClientApi, identity: &Identity) -> bool {
    let owner = api
        .access
        .as_ref()
        .map(|access| access.config().owner_email.as_str());
    identity
        .email
        .as_deref()
        .zip(owner)
        .is_some_and(|(email, owner)| email.eq_ignore_ascii_case(owner))
}

async fn guard(
    State(api): Shared,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    let admitted = match admit(&api, peer, request.headers()).await {
        Ok(admitted) => admitted,
        Err((status, message)) => return refuse(status, message),
    };
    let is_write = !matches!(*request.method(), Method::GET | Method::HEAD);
    // A guest's reads are checked here against its table; its writes by its own arm of `check_write`.
    if let Admitted::Guest(guest) = &admitted {
        if !is_write && !guests::may(guest, request.method(), request.uri().path()) {
            return refuse(StatusCode::FORBIDDEN, "a guest may not do this");
        }
    }
    if is_write {
        let path = request.uri().path().to_owned();
        if let Err((status, message)) =
            check_write(&api, &admitted, request.method(), &path, request.headers())
        {
            return refuse(status, message);
        }
    }
    note_client(&api, request.headers());
    request.extensions_mut().insert(admitted);
    next.run(request).await
}

/// Notes the app that made an admitted request, from its `x-agency-client` header: at most once an hour for
/// each version, so a client that calls often costs no write. A header that does not read is ignored; a
/// failure to note it never fails the request.
fn note_client(api: &ClientApi, headers: &HeaderMap) {
    let Some(seen) = header_text(headers, client_version::HEADER).and_then(client_version::parse)
    else {
        return;
    };
    let now = api.node.now();
    {
        let mut noted = api
            .client_notes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let key = (seen.platform, seen.version.clone());
        if noted.get(&key).is_some_and(|at| now - *at < NOTE_EVERY) {
            return;
        }
        if noted.len() >= NOTED_KEPT {
            noted.clear();
        }
        noted.insert(key, now);
    }
    if let Err(error) = clients_seen::note(&api.node, &seen) {
        (api.log)(&format!("could not record an app version ({error})"));
    }
}

/// How long a version is left before it is noted again, and how many versions the throttle remembers.
const NOTE_EVERY: Duration = Duration::hours(1);
const NOTED_KEPT: usize = 256;

/// A malformed id in the path is reported like a missing one.
fn parse_id(raw: &str) -> Result<Id, Error> {
    Id::parse(raw).map_err(|_| Error::not_found("not found"))
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

/// A JSON body, or 400 naming what it should have been.
fn json_body<T: serde::de::DeserializeOwned>(bytes: &Bytes, what: &str) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| Error::bad_request(format!("malformed {what}")))
}

/// A decision signed by one of the account's devices. The node checks and relays it; it never signs.
async fn post_envelope(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    Path((org, team)): Path<(String, String)>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let input: SignedDecisionInput = json_body(&bytes, "signed decision")?;
        submit_signed_decision(&api.node, &caller_of(&admitted), &org, &team, input).await
    };
    respond(result.await)
}

/// Who the request acts as: a guest as its hosted member, anyone else admitted as the node's own account.
fn caller_of(admitted: &Admitted) -> Caller {
    match admitted {
        Admitted::Guest(guest) => Caller::Guest(guest.clone()),
        Admitted::Local | Admitted::Access(_) => Caller::Owner,
    }
}

/// Whether the request is the node's owner's: a direct local one (the machine's own user), or one
/// through Access signed in as the configured owner. Other Access sign-ins may read this node's data
/// but are not told about its machine.
fn is_owner_request(api: &ClientApi, admitted: &Admitted) -> bool {
    match admitted {
        Admitted::Local => true,
        Admitted::Access(identity) => is_owner(api, identity),
        Admitted::Guest(_) => false,
    }
}

/// `list` with where and how to approve a device here, for the owner only: the command holds the path of
/// the running program, which can hold a user name.
fn with_node_info(api: &ClientApi, admitted: &Admitted, mut list: DeviceList) -> DeviceList {
    if !is_owner_request(api, admitted) {
        return list;
    }
    list.node = api.launch.as_ref().map(|launch| {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        NodeInfo {
            url: api.node.config().url,
            data_dir: launch.data_dir.clone(),
            command: launch.device_add_command(home.as_deref()),
        }
    });
    list
}

async fn get_devices(State(api): Shared, Extension(admitted): Extension<Admitted>) -> Response {
    if let Admitted::Guest(guest) = &admitted {
        return respond(agency_core::guest_reads::device_list(&api.node, guest));
    }
    respond(device_list(&api.node).map(|list| with_node_info(&api, &admitted, list)))
}

/// A new device asks to be added; the answer carries its pairing code.
async fn post_device_request(State(api): Shared, bytes: Bytes) -> Response {
    let result = json_body::<DeviceRequestInput>(&bytes, "device request")
        .and_then(|input| request_device(&api.node, input));
    respond(result)
}

/// A device change signed by one of the account's devices, relayed to the org's root node.
async fn post_device_change(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let signed: Signed<DeviceChange> = json_body(&bytes, "device change")?;
        agency_core::devices::submit_device_change_as(&api.node, &caller_of(&admitted), signed)
            .await
    };
    respond(result.await)
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

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub(crate) fn page_response(content_type: &str, body: Body) -> Response {
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
/// The extensions of the built client's own files.
const FILE_EXTENSIONS: [&str; 12] = [
    "html",
    "js",
    "mjs",
    "css",
    "json",
    "map",
    "webmanifest",
    "svg",
    "png",
    "ico",
    "woff2",
    "txt",
];

/// Whether a path names one of the built client's files rather than one of its routes: anything under
/// `assets/` or `icons/`, or a last part with a file's extension (`sw.js`, `manifest.webmanifest`).
fn is_file_path(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or(path);
    let has_file_extension = last
        .rsplit_once('.')
        .is_some_and(|(_, extension)| FILE_EXTENSIONS.contains(&extension));
    path.starts_with("assets/") || path.starts_with("icons/") || has_file_extension
}

/// A file of the built client, the page for one of its routes, or 404. A missing file is 404 and never
/// the page: the page carries the write token, and a cache that keeps answers by path (a service worker
/// asking for a file of an older build) must not keep it under a file's name.
async fn page(State(api): Shared, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let is_file =
        !path.is_empty() && path != "index.html" && !path.split('/').any(|part| part == "..");
    match api.assets.get(path).filter(|_| is_file) {
        Some(bytes) => page_response(content_type(path), Body::from(bytes.into_owned())),
        None if path.starts_with("api/") || path == "api" => {
            refuse(StatusCode::NOT_FOUND, "no such route")
        }
        None if is_file && is_file_path(path) => {
            let mut response = page_response("text/plain; charset=utf-8", Body::from("not found"));
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
        None => index_page(&api),
    }
}

/// The Client API and the pages. Serve it with `into_make_service_with_connect_info::<SocketAddr>()`.
/// The routes behind the guard that this module serves itself, by method and path: the one list the router
/// is built from (`method_router` gives each path its handlers) and that the tests walk.
const ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/about"),
    ("GET", "/api/me"),
    ("GET", "/api/session"),
    ("GET", "/api/decisions"),
    ("GET", "/api/devices"),
    ("GET", "/api/push/tokens"),
    ("POST", "/api/push/tokens"),
    ("DELETE", "/api/push/tokens/{token}"),
    ("GET", "/api/push/vapid"),
    ("GET", "/api/push/subscriptions"),
    ("POST", "/api/push/subscriptions"),
    ("DELETE", "/api/push/subscriptions/{id}"),
    ("POST", "/api/devices/requests"),
    ("POST", "/api/devices/changes"),
    ("GET", "/api/orgs/{org}"),
    ("GET", "/api/orgs/{org}/teams/{team}/view"),
    ("GET", "/api/orgs/{org}/teams/{team}/usage"),
    ("GET", "/api/orgs/{org}/evaluation"),
    ("GET", "/api/orgs/{org}/effort"),
    ("GET", "/api/orgs/{org}/teams/{team}/activity"),
    ("POST", "/api/orgs/{org}/teams/{team}/envelopes"),
    ("GET", HANDOFF_PATH),
    ("GET", CALLBACK_PATH),
];

/// The routes a guest may reach (`guests::GUEST_ROUTES`), by method and path, for the tests.
pub fn guest_routes() -> Vec<(&'static str, &'static str)> {
    guests::GUEST_ROUTES.to_vec()
}

/// Every route behind the guard, this module's and its parts' (outcomes, metrics, telemetry), by method and path.
pub fn routes_behind_the_guard() -> Vec<(&'static str, &'static str)> {
    let parts = outcomes::ROUTES
        .iter()
        .chain(metrics::ROUTES)
        .chain(telemetry::ROUTES);
    ROUTES.iter().chain(parts).copied().collect()
}

/// The handlers of one path of `ROUTES`, for the methods listed there.
fn method_router(path: &str) -> MethodRouter<Arc<ClientApi>> {
    match path {
        "/api/about" => get(get_about),
        "/api/me" => get(get_me),
        "/api/session" => get(get_session),
        "/api/decisions" => get(get_decisions),
        "/api/devices" => get(get_devices),
        "/api/push/tokens" => get(get_push_tokens).post(post_push_token),
        "/api/push/tokens/{token}" => delete(delete_push_token),
        "/api/push/vapid" => get(get_vapid_key),
        "/api/push/subscriptions" => get(get_push_subscriptions).post(post_push_subscription),
        "/api/push/subscriptions/{id}" => delete(delete_push_subscription),
        "/api/devices/requests" => post(post_device_request),
        "/api/devices/changes" => post(post_device_change),
        "/api/orgs/{org}" => get(get_org),
        "/api/orgs/{org}/teams/{team}/view" => get(get_team),
        "/api/orgs/{org}/teams/{team}/usage" => get(get_usage),
        "/api/orgs/{org}/evaluation" => get(get_evaluation),
        "/api/orgs/{org}/effort" => get(get_effort),
        "/api/orgs/{org}/teams/{team}/activity" => get(get_activity),
        "/api/orgs/{org}/teams/{team}/envelopes" => post(post_envelope),
        HANDOFF_PATH => get(handoff::handoff),
        CALLBACK_PATH => get(handoff::callback),
        other => unreachable!("no handlers for {other}, which ROUTES lists"),
    }
}

pub fn router(api: Arc<ClientApi>) -> Router {
    let mut paths: Vec<&str> = ROUTES.iter().map(|(_, path)| *path).collect();
    paths.dedup();
    let own = paths.into_iter().fold(Router::new(), |router, path| {
        router.route(path, method_router(path))
    });
    own.merge(outcomes::routes(api.clone()))
        .merge(metrics::routes(api.clone()))
        .merge(telemetry::routes())
        .fallback(page)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(api.clone(), guard))
        // Added after the guard, so it answers without an Access sign-in: Apple's servers fetch it.
        .route(ASSOCIATION_PATH, get(handoff::association))
        .route(REDEEM_PATH, post(handoff::redeem))
        .with_state(api)
}

#[cfg(test)]
mod tests {
    use super::content_type;

    #[test]
    fn serves_the_app_manifest_as_a_manifest() {
        assert_eq!(
            content_type("manifest.webmanifest"),
            "application/manifest+json"
        );
    }

    #[test]
    fn keeps_the_types_of_the_other_built_files() {
        assert_eq!(
            content_type("assets/index-abc.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("sw.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("offline.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("offline.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("icons/icon-192.png"), "image/png");
        assert_eq!(content_type("icons/seal.svg"), "image/svg+xml");
        assert_eq!(content_type("unknown.bin"), "application/octet-stream");
    }
}
