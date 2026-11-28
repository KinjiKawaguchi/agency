//! The route of an org's change history (docs/protocol.md, "Org definition"). The owner's alone, like the node's
//! other records about its machine and its org's numbers: the history names each key that signed and every
//! change to the people of the org, which a member Access lets in has no need to read. It answers what
//! `agency audit --json` prints, from the same function (`agency_core::audit::org_history`).
use super::{is_owner_request, refuse, respond, Admitted, ClientApi, Shared};
use agency_core::audit::org_history;
use agency_core::Error;
use agency_proto::clock;
use agency_proto::history::OrgHistory;
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

/// The routes below, by method and path, for the tests that walk every route.
pub(super) const ROUTES: &[(&str, &str)] = &[("GET", HISTORY)];

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

/// The versions of the org this node keeps, oldest first; with `since` (RFC 3339), those signed at or after it.
async fn get_history(
    api: Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let since = match query.get("since").map(|raw| clock::parse(raw)) {
        None => None,
        Some(Some(at)) => Some(at),
        Some(None) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                "since must be an RFC 3339 time, such as 2026-11-01T00:00:00Z",
            )
        }
    };
    let result = Id::parse(&org)
        .map_err(|_| Error::not_found("not found"))
        .and_then(|org| org_history(&api.node, &org, since))
        .map(|entries| OrgHistory { entries });
    respond(result)
}
