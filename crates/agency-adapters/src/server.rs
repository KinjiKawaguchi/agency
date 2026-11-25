//! The federation routes (`/fed/v1`), checked with account signatures. Each handler parses the request,
//! calls a use case in agency-core, and maps its error kind to an HTTP status.
use crate::peers::{ACCOUNT, DATE, SIGNATURE};
use agency_core::decisions::{accept_envelope, Accepted};
use agency_core::invites::accept_join;
use agency_core::orgs::{read_definition, receive};
use agency_core::read_auth::ReadAuth;
use agency_core::usage::hosted_usage;
use agency_core::views::hosted_view;
use agency_core::{Error, ErrorKind, Node};
use agency_proto::org::SignedOrg;
use agency_proto::{Envelope, Id, JoinRequest, Signed};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

type Shared = State<Arc<Node>>;

const BODY_LIMIT: usize = 256 * 1024;

pub fn status_of(kind: ErrorKind) -> StatusCode {
    match kind {
        ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
        ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::Conflict => StatusCode::CONFLICT,
        ErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn respond<T: Serialize>(result: Result<T, Error>) -> Response {
    match result {
        Ok(body) => Json(body).into_response(),
        Err(error) => (
            status_of(error.kind),
            Json(json!({ "error": error.message })),
        )
            .into_response(),
    }
}

/// A malformed id in the path is reported like a missing one.
fn parse_id(raw: &str) -> Result<Id, Error> {
    Id::parse(raw).map_err(|_| Error::not_found("not found"))
}

fn parse_body<T: DeserializeOwned>(bytes: &Bytes) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| Error::bad_request("malformed body"))
}

fn read_auth(headers: &HeaderMap) -> ReadAuth {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned()
    };
    ReadAuth {
        account: header(ACCOUNT),
        date: header(DATE),
        signature: header(SIGNATURE),
    }
}

/// An org definition, for a member's signed read or an open invite code.
async fn get_definition(
    State(node): Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let result = parse_id(&org).and_then(|org| {
        read_definition(
            &node,
            &org,
            query.get("code").map(String::as_str),
            &read_auth(&headers),
            uri.path(),
        )
    });
    respond(result)
}

async fn post_definition(State(node): Shared, Path(org): Path<String>, bytes: Bytes) -> Response {
    let result = async {
        let org = parse_id(&org)?;
        let doc: SignedOrg = parse_body(&bytes)?;
        if doc.body.org != org {
            return Err(Error::bad_request("the definition is for another org"));
        }
        receive(&node, doc).await
    };
    respond(result.await.map(|()| json!({ "accepted": true })))
}

async fn post_join(State(node): Shared, Path(org): Path<String>, bytes: Bytes) -> Response {
    let result = async {
        let org = parse_id(&org)?;
        let signed: Signed<JoinRequest> = parse_body(&bytes)?;
        accept_join(&node, &org, signed).await
    };
    respond(result.await)
}

/// The TeamView of a team on this node, for a signed read by a member of its org.
async fn get_view(
    State(node): Shared,
    Path((org, team)): Path<(String, String)>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        hosted_view(&node, &org, &team, &read_auth(&headers), uri.path()).await
    };
    respond(result.await)
}

/// The token usage of a team on this node, for a signed read by a member of its org. The signature
/// covers the path with its `days` query.
async fn get_usage(
    State(node): Shared,
    Path((org, team)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let days = query
            .get("days")
            .and_then(|raw| raw.parse::<u32>().ok())
            .ok_or_else(|| Error::bad_request("days must be a number"))?;
        hosted_usage(&node, &org, &team, days, &read_auth(&headers)).await
    };
    respond(result.await)
}

async fn post_envelope(
    State(node): Shared,
    Path((org, team)): Path<(String, String)>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let signed: Signed<Envelope> = parse_body(&bytes)?;
        accept_envelope(&node, &org, &team, signed).await
    };
    respond(result.await.map(|accepted| match accepted {
        Accepted::Applied => json!({ "applied": true }),
        Accepted::Duplicate => json!({ "duplicate": true }),
    }))
}

pub fn router(node: Arc<Node>) -> Router {
    Router::new()
        .route(
            "/fed/v1/orgs/{org}/definition",
            get(get_definition).post(post_definition),
        )
        .route("/fed/v1/orgs/{org}/join", post(post_join))
        .route("/fed/v1/teams/{org}/{team}/view", get(get_view))
        .route("/fed/v1/teams/{org}/{team}/usage", get(get_usage))
        .route("/fed/v1/teams/{org}/{team}/envelopes", post(post_envelope))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(node)
}
