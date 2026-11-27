//! What a guest (a member hosted by this node, signed in through Access) may reach of the Client API: one
//! table, and nothing outside it. Every other route answers 403 to a guest; the routes in it are scoped to the
//! guest's member in the handlers.
use agency_core::guests::Guest;
use axum::http::Method;

/// A guest's routes: `(method, path)`, where `{org}` must be the guest's own org, `{team}` any team, and
/// `{token}` or `{id}` any one segment (the handler finds only the guest's own).
pub(super) const GUEST_ROUTES: [(&str, &str); 15] = [
    ("GET", "/api/about"),
    ("GET", "/api/me"),
    ("GET", "/api/session"),
    ("GET", "/api/devices"),
    ("GET", "/api/decisions"),
    ("GET", "/api/orgs/{org}/teams/{team}/view"),
    ("POST", "/api/orgs/{org}/teams/{team}/envelopes"),
    ("POST", "/api/devices/changes"),
    ("GET", "/api/push/tokens"),
    ("POST", "/api/push/tokens"),
    ("DELETE", "/api/push/tokens/{token}"),
    ("GET", "/api/push/vapid"),
    ("GET", "/api/push/subscriptions"),
    ("POST", "/api/push/subscriptions"),
    ("DELETE", "/api/push/subscriptions/{id}"),
];

/// Whether `path` fits `pattern`, with `{org}` the guest's org and `{team}` one well-formed segment.
fn fits(pattern: &str, path: &str, guest: &Guest) -> bool {
    let (wanted, given): (Vec<&str>, Vec<&str>) =
        (pattern.split('/').collect(), path.split('/').collect());
    wanted.len() == given.len()
        && wanted.iter().zip(&given).all(|(want, part)| match *want {
            "{org}" => *part == guest.org.as_str(),
            "{team}" => agency_proto::Id::parse(part).is_ok(),
            "{token}" | "{id}" => !part.is_empty(),
            fixed => fixed == *part,
        })
}

/// Whether `path` is the web client itself (its page and files), which holds none of the node's data: not
/// under `/api`, not the node's own measurements (`/metrics`), and not the app sign-in (`/app`).
pub(super) fn is_the_client(path: &str) -> bool {
    let reserved = ["/api", "/metrics", "/app"];
    !reserved
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

/// Whether a guest may make this request at all. HEAD goes with GET. A guest loads the web client too, so
/// that its shell runs; the client reads the node only through the routes above.
pub(super) fn may(guest: &Guest, method: &Method, path: &str) -> bool {
    let method = if method == Method::HEAD {
        "GET"
    } else {
        method.as_str()
    };
    let is_page = method == "GET" && is_the_client(path);
    is_page
        || GUEST_ROUTES
            .iter()
            .any(|(allowed, pattern)| *allowed == method && fits(pattern, path, guest))
}

/// Whether the request is a write a guest may make: the write half of the table.
pub(super) fn may_write(guest: &Guest, method: &Method, path: &str) -> bool {
    method != Method::GET && method != Method::HEAD && may(guest, method, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::Id;

    fn carol() -> Guest {
        Guest::new(
            "carol@example.com",
            Id::parse("acme").unwrap(),
            Id::parse("carol").unwrap(),
        )
    }

    #[test]
    fn a_guest_reaches_its_own_org_and_nothing_else() {
        let guest = carol();
        assert!(may(&guest, &Method::GET, "/api/me"));
        assert!(may(&guest, &Method::GET, "/api/orgs/acme/teams/app/view"));
        assert!(may_write(
            &guest,
            &Method::POST,
            "/api/orgs/acme/teams/app/envelopes"
        ));
        assert!(
            !may(&guest, &Method::GET, "/api/orgs/other/teams/app/view"),
            "another org"
        );
        assert!(!may(&guest, &Method::GET, "/api/orgs/acme/teams/app/usage"));
        assert!(!may(&guest, &Method::GET, "/api/orgs/acme"));
        assert!(may_write(&guest, &Method::POST, "/api/push/tokens"));
        assert!(may_write(
            &guest,
            &Method::DELETE,
            "/api/push/subscriptions/abc"
        ));
        assert!(!may(&guest, &Method::DELETE, "/api/push/subscriptions/"));
        assert!(!may(&guest, &Method::POST, "/api/telemetry"));
        assert!(!may(&guest, &Method::DELETE, "/api/devices/changes"));
        assert!(!may(
            &guest,
            &Method::GET,
            "/api/orgs/acme/teams/app/view/extra"
        ));
        assert!(!may_write(&guest, &Method::GET, "/api/me"));
        assert!(may(&guest, &Method::GET, "/"), "the web client");
        assert!(may(&guest, &Method::GET, "/desk"));
        assert!(may(&guest, &Method::GET, "/assets/index.js"));
        assert!(!may(&guest, &Method::GET, "/metrics"));
        assert!(!may(&guest, &Method::GET, "/app/handoff"));
        assert!(!may(&guest, &Method::GET, "/api"));
        assert!(
            !may(&guest, &Method::POST, "/desk"),
            "only reading the client"
        );
    }
}
