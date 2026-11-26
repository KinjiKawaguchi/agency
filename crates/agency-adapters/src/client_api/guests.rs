//! What a guest (a member hosted by this node, signed in through Access) may reach of the Client API: one
//! table, and nothing outside it. Every other route answers 403 to a guest; the routes in it are scoped to the
//! guest's member in the handlers.
use agency_core::guests::Guest;
use axum::http::Method;

/// A guest's routes: `(method, path)`, where `{org}` must be the guest's own org and `{team}` any team.
pub(super) const GUEST_ROUTES: [(&str, &str); 8] = [
    ("GET", "/api/about"),
    ("GET", "/api/me"),
    ("GET", "/api/session"),
    ("GET", "/api/devices"),
    ("GET", "/api/decisions"),
    ("GET", "/api/orgs/{org}/teams/{team}/view"),
    ("POST", "/api/orgs/{org}/teams/{team}/envelopes"),
    ("POST", "/api/devices/changes"),
];

/// Whether `path` fits `pattern`, with `{org}` the guest's org and `{team}` one well-formed segment.
fn fits(pattern: &str, path: &str, guest: &Guest) -> bool {
    let (wanted, given): (Vec<&str>, Vec<&str>) =
        (pattern.split('/').collect(), path.split('/').collect());
    wanted.len() == given.len()
        && wanted.iter().zip(&given).all(|(want, part)| match *want {
            "{org}" => *part == guest.org.as_str(),
            "{team}" => agency_proto::Id::parse(part).is_ok(),
            fixed => fixed == *part,
        })
}

/// Whether a guest may make this request at all. HEAD goes with GET.
pub(super) fn may(guest: &Guest, method: &Method, path: &str) -> bool {
    let method = if method == Method::HEAD {
        "GET"
    } else {
        method.as_str()
    };
    GUEST_ROUTES
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
        assert!(!may(&guest, &Method::POST, "/api/push/tokens"));
        assert!(!may(&guest, &Method::DELETE, "/api/devices/changes"));
        assert!(!may(
            &guest,
            &Method::GET,
            "/api/orgs/acme/teams/app/view/extra"
        ));
        assert!(!may_write(&guest, &Method::GET, "/api/me"));
    }
}
