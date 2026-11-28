//! The route of an org's people for its owner (docs/protocol.md, "Members, devices, and open seats"). The
//! owner is the direct local request, or the Access sign-in of the configured owner: a guest does not reach
//! it (the guard), and no other sign-in Access lets in does either.
use super::{is_owner_request, parse_id, refuse, respond, Admitted, ClientApi, Route, Shared};
use agency_core::org_members::org_members;
use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;

/// The routes below.
pub(super) const ROUTES: &[Route] = &[Route::get("/api/orgs/{org}/members").reported()];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new().route(ROUTES[0].path, get(get_members))
}

async fn get_members(
    api: Shared,
    Extension(admitted): Extension<Admitted>,
    Path(org): Path<String>,
) -> Response {
    if !is_owner_request(&api, &admitted) {
        return refuse(
            StatusCode::FORBIDDEN,
            "only the node's owner may read the org's members",
        );
    }
    respond(parse_id(&org).and_then(|org| org_members(&api.node, &org)))
}
