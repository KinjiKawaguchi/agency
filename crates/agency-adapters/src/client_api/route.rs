//! A route of the Client API, declared once beside the handlers that serve it: its method, its path, and who
//! besides the owner may reach it and whether a client may report its timing. The guard's tables, the routes
//! a client may report and `testdata/route-templates.json` are read off these declarations, so a route is
//! added in one place.

/// One route behind the guard: `method` and `path` as the router has them (`{org}`, `{team}`, `{id}` and
/// `{token}` stand for one segment each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub method: &'static str,
    pub path: &'static str,
    is_for_guests: bool,
    is_for_claimants: bool,
    is_reported: bool,
}

impl Route {
    const fn new(method: &'static str, path: &'static str) -> Self {
        Self {
            method,
            path,
            is_for_guests: false,
            is_for_claimants: false,
            is_reported: false,
        }
    }

    pub const fn get(path: &'static str) -> Self {
        Self::new("GET", path)
    }

    pub const fn post(path: &'static str) -> Self {
        Self::new("POST", path)
    }

    pub const fn delete(path: &'static str) -> Self {
        Self::new("DELETE", path)
    }

    /// The same route, which a guest may reach too (`guests`), scoped to its member in the handler.
    pub const fn for_guests(self) -> Self {
        Self {
            is_for_guests: true,
            ..self
        }
    }

    /// The same route, which a claimant may reach too (`claimants`).
    pub const fn for_claimants(self) -> Self {
        Self {
            is_for_claimants: true,
            ..self
        }
    }

    /// The same route, whose timing a client may report (`POST /api/telemetry`).
    pub const fn reported(self) -> Self {
        Self {
            is_reported: true,
            ..self
        }
    }

    pub fn is_for_guests(&self) -> bool {
        self.is_for_guests
    }

    pub fn is_for_claimants(&self) -> bool {
        self.is_for_claimants
    }

    pub fn is_reported(&self) -> bool {
        self.is_reported
    }

    /// `<method> <path>`, the name a client gives the route in a report.
    pub fn template(&self) -> String {
        format!("{} {}", self.method, self.path)
    }

    /// `(method, path)`, as the tests that walk the routes take them.
    pub fn pair(&self) -> (&'static str, &'static str) {
        (self.method, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::Route;

    #[test]
    fn a_route_is_reached_by_the_owner_alone_until_it_says_otherwise() {
        let plain = Route::get("/api/me");
        assert!(!plain.is_for_guests() && !plain.is_for_claimants() && !plain.is_reported());
        let marked = Route::post("/api/claim").for_claimants().reported();
        assert!(marked.is_for_claimants() && marked.is_reported() && !marked.is_for_guests());
        assert!(Route::delete("/api/x").for_guests().is_for_guests());
        assert_eq!(marked.template(), "POST /api/claim");
        assert_eq!(marked.pair(), ("POST", "/api/claim"));
    }
}
