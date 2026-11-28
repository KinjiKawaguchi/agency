//! What a guest (a member hosted by this node, signed in through Access) may reach of the Client API: the
//! routes declared `for_guests`, and nothing else. Every other route answers 403 to a guest; those routes are
//! scoped to the guest's member in the handlers.
use agency_core::guests::Guest;
use axum::http::Method;

/// Whether `path` fits `pattern`, the path of a route declared `for_guests`: `{org}` must be the guest's own
/// org, `{team}` one well-formed segment, and `{token}` or `{id}` any one segment (the handler finds only the
/// guest's own).
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
/// that its shell runs; the client reads the node only through a guest's routes.
pub(super) fn may(guest: &Guest, method: &Method, path: &str) -> bool {
    let method = if method == Method::HEAD {
        "GET"
    } else {
        method.as_str()
    };
    let is_page = method == "GET" && is_the_client(path);
    is_page
        || super::declared().any(|route| {
            route.is_for_guests() && route.method == method && fits(route.path, path, guest)
        })
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
