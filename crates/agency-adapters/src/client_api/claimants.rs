//! What a claimant may reach of the Client API: a person who signed in to Access with the address of a claim
//! link, before they are a guest. One table, and nothing outside it: the claim routes and the web client's own
//! pages. Every other route answers 403.
use axum::http::Method;

/// A claimant's routes: `(method, path)`.
pub(super) const CLAIMANT_ROUTES: [(&str, &str); 2] =
    [("GET", "/api/claim"), ("POST", "/api/claim")];

/// Whether a claimant may make this request at all. HEAD goes with GET. The web client's pages hold none of
/// the node's data, so the claim page can load; it reads the node only through the routes above.
pub(super) fn may(method: &Method, path: &str) -> bool {
    let method = if method == Method::HEAD {
        "GET"
    } else {
        method.as_str()
    };
    let is_page = method == "GET" && super::guests::is_the_client(path);
    is_page
        || CLAIMANT_ROUTES
            .iter()
            .any(|(allowed, route)| *allowed == method && *route == path)
}

/// Whether the request is a write a claimant may make: the write half of the table.
pub(super) fn may_write(method: &Method, path: &str) -> bool {
    method != Method::GET && method != Method::HEAD && may(method, path)
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
