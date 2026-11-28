//! What a claimant may reach of the Client API: a person who signed in to Access with the address of a claim
//! link, before they are a guest. The routes declared `for_claimants` (the claim routes) and the web client's
//! own pages, and nothing else: every other route answers 403.
use super::{json_body, refuse, respond, Admitted, ClientApi, Route, Shared};
use agency_proto::claim::ClaimInput;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Extension, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;

const CLAIM: &str = "/api/claim";

pub(super) const ROUTES: &[Route] = &[
    Route::get(CLAIM).for_claimants().reported(),
    Route::post(CLAIM).for_claimants().reported(),
];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new().route(CLAIM, get(get_claim).post(post_claim))
}

/// Whether a claimant may make this request at all. HEAD goes with GET. The web client's pages hold none of
/// the node's data, so the claim page can load; it reads the node only through the claim routes.
pub(super) fn may(method: &Method, path: &str) -> bool {
    let method = if method == Method::HEAD {
        "GET"
    } else {
        method.as_str()
    };
    let is_page = method == "GET" && super::guests::is_the_client(path);
    is_page
        || super::declared()
            .any(|route| route.is_for_claimants() && route.method == method && route.path == path)
}

/// Whether the request is a write a claimant may make: the write half of the table.
pub(super) fn may_write(method: &Method, path: &str) -> bool {
    method != Method::GET && method != Method::HEAD && may(method, path)
}

/// How many redeems of a claim link one client address may try in a minute.
const CLAIM_TRIES_PER_MINUTE: usize = 20;

/// Where the claim of the person who signed in with the address of a claim link stands.
async fn get_claim(State(api): Shared, Extension(admitted): Extension<Admitted>) -> Response {
    let Admitted::Claimant(email) = &admitted else {
        // The owner and a local request have no claim: there is nothing here, as for an address that redeemed none.
        return refuse(StatusCode::NOT_FOUND, "no claim");
    };
    respond(agency_core::claims::state(&api.node, email))
}

/// A claimant's browser redeems the code of its link with the device key it made. The write token and the JSON body
/// are checked before, as for every write; the authority is the Access sign-in and the code. Each client address gets
/// a few tries a minute, even with a good code, from the table the app's sign-in code has (`attempts`).
async fn post_claim(
    State(api): Shared,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Extension(admitted): Extension<Admitted>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    let Admitted::Claimant(email) = &admitted else {
        return refuse(StatusCode::NOT_FOUND, "no claim");
    };
    let client = crate::handoff::client_of(peer, &headers);
    let tried = api.claim_attempts.lock().map_or(Ok(()), |mut table| {
        table.try_once(api.node.now(), &client, CLAIM_TRIES_PER_MINUTE)
    });
    if let Err(seconds) = tried {
        let mut limited = refuse(StatusCode::TOO_MANY_REQUESTS, "too many tries");
        if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
            limited.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return limited;
    }
    let result = match json_body::<ClaimInput>(&bytes, "claim") {
        Ok(input) => {
            agency_core::claims::redeem(
                &api.node,
                &input.code,
                email,
                &input.public_key,
                &input.label,
            )
            .await
        }
        Err(error) => Err(error),
    };
    respond(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claimant_reaches_the_claim_and_the_pages_and_nothing_else() {
        assert!(may(&Method::GET, "/api/claim"));
        assert!(may_write(&Method::POST, "/api/claim"));
        assert!(may(&Method::GET, "/"));
        assert!(may(&Method::GET, "/claim/continue"));
        for (method, path) in [
            (Method::GET, "/api/me"),
            (Method::GET, "/api/claim/extra"),
            (Method::PUT, "/api/claim"),
            (Method::DELETE, "/api/claim"),
            (Method::POST, "/api/devices/changes"),
            (Method::GET, "/metrics"),
            (Method::GET, "/app/handoff"),
        ] {
            assert!(!may(&method, path), "{method} {path}");
        }
        assert!(!may_write(&Method::GET, "/api/claim"));
    }
}
