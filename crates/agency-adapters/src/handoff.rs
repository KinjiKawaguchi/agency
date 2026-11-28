//! Signing in to Access outside the Apple apps' web view (docs/protocol.md, "App sign-in"). The app opens
//! `/app/handoff?state=<s>&challenge=<c>&via=<link|scheme>` in a system browser session, where passkeys
//! work; Access signs the owner in and the node answers with a redirect to one of two fixed callbacks, with
//! a single-use `code` and the app's `state` in the fragment: no token. The app then redeems the code with
//! the verifier whose SHA-256 is `challenge` (`POST /app/redeem`), and only then gets the Access JWT. A
//! callback that someone else read is useless without the verifier, which never left the app.
//!
//! Nothing here is logged: the request URL holds only `state`, `challenge` and `via`, the callback holds
//! a code that is worth nothing alone, and the token appears only in the answer to the redeem.
use crate::attempts::Attempts;
use crate::client_api::{
    header_text, is_owner, is_same_secret, page_response, refuse, Admitted, ClientApi, Shared,
};
use agency_core::config::AccessConfig;
use agency_proto::clock::{Duration, OffsetDateTime};
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;

pub const HANDOFF_PATH: &str = "/app/handoff";
pub const CALLBACK_PATH: &str = "/app/callback";
pub const REDEEM_PATH: &str = "/app/redeem";
pub const ASSOCIATION_PATH: &str = "/.well-known/apple-app-site-association";
/// The scheme of the callback for a host the app has no universal link for. It carries no secret alone.
pub const CALLBACK_SCHEME: &str = "agency-signin";
/// The header Access sends the verified JWT in.
pub(crate) const ACCESS_HEADER: &str = "cf-access-jwt-assertion";
/// 32 random bytes (or a SHA-256) as unpadded base64url: the length of a state, a challenge, a code, and a verifier.
const SECRET_LENGTH: usize = 43;
/// How long a code can be redeemed.
const CODE_LIFETIME_SECONDS: i64 = 60;
/// The most codes waiting to be redeemed: a sign-in is the owner's, so this is generous.
const MOST_PENDING: usize = 64;
/// How many redeems one client address may try in a minute (the table of addresses is `attempts`).
const REDEEMS_PER_MINUTE: usize = 20;
/// Why `via=scheme` is refused: the answer a browser session shows the person who sees it.
const SCHEME_REFUSED: &str = "this node signs the app in only through the universal link of its own host; the custom scheme is off (access.allowSchemeSignIn)";
const CALLBACK_PAGE: &str =
    "<!doctype html><meta charset=\"utf-8\"><title>Agency</title><p>Return to the Agency app.</p>";

/// The app identifier, `<team id>.<bundle id>`: ten uppercase letters or digits, a dot, and a bundle
/// id of letters, digits, dots, and hyphens. A malformed value counts as none.
pub fn app_id(config: &AccessConfig) -> Option<&str> {
    let id = config.app_id.as_deref()?;
    let (team, bundle) = id.split_once('.')?;
    let is_team = team.len() == 10
        && team
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    let is_bundle = (1..=155).contains(&bundle.len())
        && bundle
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !bundle.starts_with('.')
        && !bundle.ends_with('.');
    (is_team && is_bundle).then_some(id)
}

fn configured_app(api: &ClientApi) -> Option<(&AccessConfig, &str)> {
    let config = api.access.as_ref()?.provider().access_config()?;
    Some((config, app_id(config)?))
}

fn is_secret(text: &str) -> bool {
    text.len() == SECRET_LENGTH
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Where the callback goes: the universal link on the node's own host, or the app's custom scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    Link,
    Scheme,
}

/// The query must be exactly `state=<43>&challenge=<43>&via=link|scheme`, in that order; the alphabet leaves no
/// room for another parameter or an escape, and nothing of it chooses where the answer goes but the two fixed
/// callbacks.
fn query_of(uri: &Uri) -> Option<(&str, &str, Via)> {
    let mut parts = uri.query()?.split('&');
    let state = parts.next()?.strip_prefix("state=")?;
    let challenge = parts.next()?.strip_prefix("challenge=")?;
    let via = match parts.next()?.strip_prefix("via=")? {
        "link" => Via::Link,
        "scheme" => Via::Scheme,
        _ => return None,
    };
    let is_exact = parts.next().is_none() && is_secret(state) && is_secret(challenge);
    is_exact.then_some((state, challenge, via))
}

/// The form of the handoff before the code exchange: the query is exactly `state=<43>`. It is answered as
/// before, with the token in the fragment, until the apps in use speak the exchange (deprecated, docs/protocol.md).
fn legacy_state_of(uri: &Uri) -> Option<&str> {
    let state = uri.query()?.strip_prefix("state=")?;
    is_secret(state).then_some(state)
}

struct Pending {
    token: String,
    challenge: String,
    expires: OffsetDateTime,
}

#[derive(Default)]
struct Pool {
    pending: HashMap<String, Pending>,
    /// The tries of each client address to redeem, within the minute.
    attempts: Attempts,
}

/// The codes made by `/app/handoff` and waiting to be redeemed, in memory: a restart of the node ends them,
/// which costs the owner one more sign-in.
#[derive(Default)]
pub struct Codes(Mutex<Pool>);

/// What a redeem came to.
enum Redeemed {
    Token(String),
    /// Unknown, used, expired, malformed, or the wrong verifier: one answer for all, so the answer teaches
    /// nothing about which.
    Invalid,
    /// Too many tries from this address; seconds until one more is allowed.
    Limited(i64),
}

impl Codes {
    /// A fresh code for `token`, valid for a minute, bound to `challenge`; `None` when too many wait.
    fn mint(&self, now: OffsetDateTime, token: &str, challenge: &str) -> Option<String> {
        let mut pool = self.0.lock().ok()?;
        pool.pending.retain(|_, one| one.expires > now);
        if pool.pending.len() >= MOST_PENDING {
            return None;
        }
        let mut bytes = [0u8; 32];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
        let code = URL_SAFE_NO_PAD.encode(bytes);
        pool.pending.insert(
            code.clone(),
            Pending {
                token: token.to_owned(),
                challenge: challenge.to_owned(),
                expires: now + Duration::seconds(CODE_LIFETIME_SECONDS),
            },
        );
        Some(code)
    }

    /// Redeems `code` with `verifier` for a client address. The code is taken out whatever the verifier says,
    /// so a code is tried once; the token goes only to the one whose verifier hashes to the challenge.
    fn redeem(&self, now: OffsetDateTime, client: &str, code: &str, verifier: &str) -> Redeemed {
        let Ok(mut pool) = self.0.lock() else {
            return Redeemed::Invalid;
        };
        if let Err(wait) = pool.attempts.try_once(now, client, REDEEMS_PER_MINUTE) {
            return Redeemed::Limited(wait);
        }
        pool.pending.retain(|_, one| one.expires > now);
        let (true, true) = (is_secret(code), is_secret(verifier)) else {
            return Redeemed::Invalid;
        };
        let Some(pending) = pool.pending.remove(code) else {
            return Redeemed::Invalid;
        };
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        if is_same_secret(&challenge, &pending.challenge) {
            Redeemed::Token(pending.token)
        } else {
            Redeemed::Invalid
        }
    }
}

/// Redirects the owner's verified Access session to one of the app's two fixed callbacks with a single-use
/// code. The target is built only from the configured public host or the fixed scheme; the request contributes
/// nothing but `state`, the code's challenge, and which of the two.
pub(crate) async fn handoff(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let Some((config, _)) = configured_app(&api) else {
        return refuse(StatusCode::NOT_FOUND, "no such route");
    };
    let Admitted::Access(identity) = admitted else {
        return refuse(
            StatusCode::FORBIDDEN,
            "the handoff needs a sign-in through Cloudflare Access",
        );
    };
    if !is_owner(&api, &identity) {
        return refuse(StatusCode::FORBIDDEN, "only the node's owner may sign in");
    }
    let exchange = query_of(&uri);
    // The two forms exclude each other: the old one is exactly `state=<43>`, which has no `&`.
    let legacy = legacy_state_of(&uri);
    if exchange.is_none() && legacy.is_none() {
        return refuse(
            StatusCode::BAD_REQUEST,
            "the query must be state, challenge (43 base64url characters each) and via (link or scheme)",
        );
    }
    let Some(token) = header_text(&headers, ACCESS_HEADER) else {
        return refuse(StatusCode::FORBIDDEN, "no Access token");
    };
    let location = match (exchange, legacy) {
        (Some((state, challenge, via)), _) => {
            if via == Via::Scheme && !config.allow_scheme_sign_in {
                return refuse(StatusCode::BAD_REQUEST, SCHEME_REFUSED);
            }
            let Some(code) = api.handoff.mint(api.node.now(), token, challenge) else {
                return refuse(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "too many sign-ins are waiting",
                );
            };
            match via {
                Via::Link => format!(
                    "https://{}{CALLBACK_PATH}#code={code}&state={state}",
                    config.public_host
                ),
                Via::Scheme => format!("{CALLBACK_SCHEME}://callback#code={code}&state={state}"),
            }
        }
        (None, Some(state)) => {
            // Deprecated: the token travels in the fragment of the universal link. Logged so that it is known when
            // no app uses it any more; the line holds no URL, no token and no state.
            eprintln!(
                "agency: the app handoff was used in its deprecated form (state only, no code exchange)"
            );
            format!(
                "https://{}{CALLBACK_PATH}#token={token}&state={state}",
                config.public_host
            )
        }
        (None, None) => return refuse(StatusCode::BAD_REQUEST, "no handoff"),
    };
    let Ok(location) = HeaderValue::from_str(&location) else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cannot hand the session over",
        );
    };
    let mut response = StatusCode::SEE_OTHER.into_response();
    let headers = response.headers_mut();
    headers.insert(header::LOCATION, location);
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RedeemInput {
    code: String,
    verifier: String,
}

/// The address a redeem is counted against. The node is reached through a tunnel on this machine, so the peer
/// is the tunnel and Cloudflare says who the client is; a peer that is not on this machine is its own client.
pub(crate) fn client_of(peer: SocketAddr, headers: &HeaderMap) -> String {
    if peer.ip().is_loopback() {
        let named =
            header_text(headers, "cf-connecting-ip").and_then(|ip| ip.parse::<IpAddr>().ok());
        return named.map_or_else(|| "unknown".to_owned(), |ip| ip.to_string());
    }
    peer.ip().to_string()
}

/// Exchanges a code and the verifier for the Access token the handoff made it for. It is not behind Access: the
/// app has no Access session (the token is what it is asking for), and what admits it is the code, which only
/// the browser session that signed the owner in was given, and the verifier, which only the app has. Every
/// way to fail gets the same answer; a code is tried once; each client address gets a few tries a minute.
pub(crate) async fn redeem(
    State(api): Shared,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if configured_app(&api).is_none() {
        return refuse(StatusCode::NOT_FOUND, "no such route");
    }
    let client = client_of(peer, &headers);
    let input = serde_json::from_slice::<RedeemInput>(&body).ok();
    let (code, verifier) = input
        .as_ref()
        .map_or(("", ""), |one| (one.code.as_str(), one.verifier.as_str()));
    let mut response = match api.handoff.redeem(api.node.now(), &client, code, verifier) {
        Redeemed::Token(token) => {
            (StatusCode::OK, axum::Json(json!({ "token": token }))).into_response()
        }
        Redeemed::Invalid => refuse(StatusCode::BAD_REQUEST, "the code is not valid"),
        Redeemed::Limited(seconds) => {
            let mut limited = refuse(StatusCode::TOO_MANY_REQUESTS, "too many tries");
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                limited.headers_mut().insert(header::RETRY_AFTER, value);
            }
            limited
        }
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// The callback is meant for the app, which takes it before it loads. If a browser loads it anyway, it
/// gets a page with no script that reads nothing from the URL.
pub(crate) async fn callback() -> Response {
    page_response("text/html; charset=utf-8", Body::from(CALLBACK_PAGE))
}

/// The association file Apple's servers fetch without signing in. It is fixed JSON from the app
/// identifier setting; without the setting there is no such file.
pub(crate) async fn association(State(api): Shared) -> Response {
    let Some((_, id)) = configured_app(&api) else {
        return refuse(StatusCode::NOT_FOUND, "no such route");
    };
    let body = json!({
        "applinks": { "details": [{ "appIDs": [id], "components": [{ "/": CALLBACK_PATH }] }] },
        "webcredentials": { "apps": [id] },
    });
    let headers = [
        (header::CONTENT_TYPE, "application/json"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ];
    (StatusCode::OK, headers, body.to_string()).into_response()
}
