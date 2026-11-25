//! Signing in to Access outside the iOS app's web view (docs/protocol.md, "App sign-in"). The app opens
//! `/app/handoff?state=<s>` in a system browser session, where passkeys work; Access signs the owner in
//! and the node answers with a redirect to the fixed callback `/app/callback`, carrying the Access JWT it
//! has just verified and the app's `state` in the fragment. The callback is a universal link, so only the
//! app named in the association file receives it.
//!
//! Nothing here is logged: the request URL holds only `state`, and the token appears only in the
//! `Location` header of the answer.
use crate::client_api::{
    header_text, is_owner, page_response, refuse, Admitted, ClientApi, Shared,
};
use agency_core::config::AccessConfig;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use serde_json::json;

pub const HANDOFF_PATH: &str = "/app/handoff";
pub const CALLBACK_PATH: &str = "/app/callback";
pub const ASSOCIATION_PATH: &str = "/.well-known/apple-app-site-association";
/// The header Access sends the verified JWT in.
pub(crate) const ACCESS_HEADER: &str = "cf-access-jwt-assertion";
/// 32 random bytes as unpadded base64url.
const STATE_LENGTH: usize = 43;
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
    let config = api.access.as_ref()?.config();
    Some((config, app_id(config)?))
}

/// The query must be exactly `state=<43 base64url characters>`; the alphabet leaves no room for a second
/// parameter or an escape.
fn state_of(uri: &Uri) -> Option<&str> {
    let state = uri.query()?.strip_prefix("state=")?;
    let is_state = state.len() == STATE_LENGTH
        && state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    is_state.then_some(state)
}

/// Redirects the owner's verified Access session to the app's callback. The target is built only from
/// the configured public host; the request contributes nothing but `state`.
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
    let Some(state) = state_of(&uri) else {
        return refuse(
            StatusCode::BAD_REQUEST,
            "state must be 43 base64url characters",
        );
    };
    let Some(token) = header_text(&headers, ACCESS_HEADER) else {
        return refuse(StatusCode::FORBIDDEN, "no Access token");
    };
    let location = format!(
        "https://{}{CALLBACK_PATH}#token={token}&state={state}",
        config.public_host
    );
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
