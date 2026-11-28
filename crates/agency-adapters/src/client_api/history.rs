//! The route of an org's change history (docs/protocol.md, "Org definition"). The owner's alone, like the node's
//! other records about its machine and its org's numbers: the history names each key that signed and every
//! change to the people of the org, which a member Access lets in has no need to read. It answers what
//! `agency audit --json` prints, from the same function (`agency_core::audit::org_history`).
use super::{is_owner_request, refuse, respond, Admitted, ClientApi, Route, Shared};
use agency_core::audit::{org_history_page, HistoryQuery};
use agency_core::Error;
use agency_proto::clock;
use agency_proto::history::{OrgHistory, DEFAULT_LIMIT, MOST_LIMIT};
use agency_proto::Id;
use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;

const HISTORY: &str = "/api/orgs/{org}/history";

/// The routes below.
pub(super) const ROUTES: &[Route] = &[Route::get(HISTORY).reported()];

/// Lets the owner through and refuses everyone else.
async fn require_owner(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    request: Request,
    next: Next,
) -> Response {
    if !is_owner_request(&api, &admitted) {
        return refuse(
            StatusCode::FORBIDDEN,
            "only the node's owner may read the org's history",
        );
    }
    next.run(request).await
}

pub(super) fn routes(api: Arc<ClientApi>) -> Router<Arc<ClientApi>> {
    Router::new()
        .route(HISTORY, get(get_history))
        .route_layer(middleware::from_fn_with_state(api, require_owner))
}

/// The versions of the org this node keeps, oldest first: those signed at or after `since` (RFC 3339), above
/// `after` (a version number), at most `limit` (1 to 1,000, default 200); `more` says the answer was cut.
async fn get_history(
    api: Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let asked = match asked(&query) {
        Ok(asked) => asked,
        Err(message) => return refuse(StatusCode::BAD_REQUEST, message),
    };
    let result = Id::parse(&org)
        .map_err(|_| Error::not_found("not found"))
        .and_then(|org| org_history_page(&api.node, &org, &asked))
        .map(|page| OrgHistory {
            entries: page.entries,
            more: page.is_cut,
        });
    respond(result)
}

/// What the query string asks for, or why it is refused.
fn asked(query: &HashMap<String, String>) -> Result<HistoryQuery, &'static str> {
    let since = match query.get("since") {
        None => None,
        Some(raw) => Some(
            clock::parse(raw)
                .ok_or("since must be an RFC 3339 time, such as 2026-11-01T00:00:00Z")?,
        ),
    };
    let after = match query.get("after") {
        None => 0,
        Some(raw) => raw.parse().map_err(|_| "after must be a version number")?,
    };
    let limit = match query.get("limit") {
        None => DEFAULT_LIMIT,
        Some(raw) => raw
            .parse::<usize>()
            .ok()
            .filter(|limit| (1..=MOST_LIMIT).contains(limit))
            .ok_or("limit must be a number from 1 to 1000")?,
    };
    Ok(HistoryQuery {
        since,
        after,
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn a_read_that_asks_for_nothing_gets_the_whole_history_up_to_the_default_limit() {
        let asked = asked(&query(&[])).unwrap();
        assert!(asked.since.is_none());
        assert_eq!((asked.after, asked.limit), (0, DEFAULT_LIMIT));
        assert_eq!((DEFAULT_LIMIT, MOST_LIMIT), (200, 1000));
    }

    #[test]
    fn the_limit_is_one_to_the_most_and_the_rest_is_refused() {
        assert_eq!(asked(&query(&[("limit", "1")])).unwrap().limit, 1);
        assert_eq!(
            asked(&query(&[("limit", "1000")])).unwrap().limit,
            MOST_LIMIT
        );
        for bad in ["0", "1001", "-1", "x", ""] {
            assert!(asked(&query(&[("limit", bad)])).is_err(), "{bad:?}");
        }
        assert_eq!(asked(&query(&[("after", "7")])).unwrap().after, 7);
        assert!(asked(&query(&[("after", "-1")])).is_err());
    }
}
