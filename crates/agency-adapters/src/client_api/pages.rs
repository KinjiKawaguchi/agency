//! The web client's own files and its page, which carries the write token.
use super::{refuse, ClientApi, Shared};
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

/// The attribute of `<meta name="agency-token">` that the web client's build leaves for the node to fill in.
const TOKEN_PLACEHOLDER: &str = "content=\"__AGENCY_TOKEN__\"";
const PAGE_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const NOT_BUILT: &str = "<!doctype html><meta charset=\"utf-8\"><title>Agency</title><p>The web client is not built into this node. Run <code>npm run build</code> in <code>web/</code>, then rebuild the node.</p>";

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
pub(super) async fn page(State(api): Shared, uri: Uri) -> Response {
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
