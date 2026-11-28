//! The owner's side of a claim (docs/protocol.md, "Claim link"): the claims that wait for a person's browser to be
//! confirmed, listed for the Devices screen to compare their pairing codes, and confirmed or refused. They call the
//! use cases the terminal's `agency claims` calls. The routes are not in the table of a guest or a claimant, so the
//! guard answers either 403; a write has the write token and the owner's sign-in like any other.
use super::{json_body, respond, ClientApi, Route, Shared};
use agency_core::claims::{self, ClientConfirmation};
use agency_core::Error;
use agency_proto::claim::{
    claim_id, parse_claim_id, ClaimConfirmInput, ClaimConfirmed, ClaimList, WaitingClaim,
};
use agency_proto::Timestamp;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

pub(super) const ROUTES: &[Route] = &[
    Route::get("/api/claims").reported(),
    Route::post("/api/claims/{id}/confirm").reported(),
    Route::post("/api/claims/{id}/reject").reported(),
];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new()
        .route(ROUTES[0].path, get(get_claims))
        .route(ROUTES[1].path, post(post_confirm))
        .route(ROUTES[2].path, post(post_reject))
}

/// The org and seat a path's claim id names; not found for a text that is none, as for a claim that is not there.
fn named(raw: &str) -> Result<(agency_proto::Id, agency_proto::Id), Error> {
    parse_claim_id(raw).ok_or_else(|| Error::not_found("no such claim"))
}

fn waiting(node: &agency_core::Node) -> Result<ClaimList, Error> {
    let claims = claims::pending(node)
        .into_iter()
        .map(|claim| {
            let redeemed_at = Timestamp::parse(&claim.redeemed_at)
                .map_err(|_| Error::internal("a claim's time is malformed"))?;
            Ok(WaitingClaim {
                id: claim_id(&claim.org, &claim.seat),
                org: claim.org,
                seat: claim.seat,
                seat_label: claim.seat_label,
                device_label: claim.device_label,
                pairing_code: claim.pairing_code,
                redeemed_at,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(ClaimList { claims })
}

async fn get_claims(State(api): Shared) -> Response {
    respond(waiting(&api.node))
}

/// 200 with the version that made the seat a member, or 202 with the handle of the confirmation held for the root
/// key's presence (`agency root-sign`).
async fn post_confirm(State(api): Shared, Path(raw): Path<String>, bytes: Bytes) -> Response {
    let result = async {
        let (org, seat) = named(&raw)?;
        let input: ClaimConfirmInput = json_body(&bytes, "claim confirmation")?;
        let confirmed =
            claims::confirm_from_client(&api.node, &org, &seat, &input.pairing_code).await?;
        Ok((org, seat, confirmed))
    };
    match result.await {
        Ok((org, seat, ClientConfirmation::Signed(doc))) => respond(Ok(ClaimConfirmed {
            org,
            seat,
            version: doc.body.version,
        })),
        Ok((_, _, ClientConfirmation::Held(held))) => {
            super::json_response(StatusCode::ACCEPTED, &held)
        }
        Err(error) => respond::<()>(Err(error)),
    }
}

async fn post_reject(State(api): Shared, Path(raw): Path<String>) -> Response {
    let result = async {
        let (org, seat) = named(&raw)?;
        claims::reject(&api.node, &org, &seat).await
    };
    match result.await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => respond::<()>(Err(error)),
    }
}
