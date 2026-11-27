//! Requests whose path could be read as another path: one with an encoded separator (`%2F`, `%5C`), a
//! backslash, or a dot segment (`.` or `..`, encoded or not). Each layer in front of the node (Cloudflare's
//! rules, a proxy) and the router could resolve such a path differently, so a rule that lets one path
//! through would let another; the node answers 400 before any route sees it.
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Whether a raw request path (as sent, still percent-encoded) holds an encoded separator, a backslash, or a
/// dot segment.
pub fn is_ambiguous(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("%2f") || lower.contains("%5c") || lower.contains('\\') {
        return true;
    }
    lower
        .split('/')
        .map(|segment| segment.replace("%2e", "."))
        .any(|segment| segment == "." || segment == "..")
}

/// Answers 400 to a request whose path `is_ambiguous`, before routing.
pub async fn refuse_ambiguous_paths(request: Request, next: Next) -> Response {
    if is_ambiguous(request.uri().path()) {
        let body = axum::Json(serde_json::json!({ "error": "the path is not one the node reads" }));
        return (StatusCode::BAD_REQUEST, body).into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::is_ambiguous;

    #[test]
    fn encoded_separators_backslashes_and_dot_segments_are_ambiguous() {
        for raw in [
            "/fed/..%2fapi/me",
            "/fed/..%2Fapi/me",
            "/fed/%2e%2e/api/me",
            "/fed/%2E./api/me",
            "/fed/v1/../api/me",
            "/fed/./v1",
            "/api%5cme",
            "/api\\me",
            "/fed/v1/teams/acme%2fapp/view",
            "/..",
        ] {
            assert!(is_ambiguous(raw), "{raw}");
        }
        for raw in [
            "/",
            "/api/me",
            "/fed/v1/teams/acme/app/view",
            "/assets/index.abc123.js",
            "/fed/v1/orgs/acme/joins/a.b",
            "/api/push/tokens/%41",
            "/...",
        ] {
            assert!(!is_ambiguous(raw), "{raw}");
        }
    }
}
