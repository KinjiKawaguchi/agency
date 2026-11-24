//! 台どうしの道 (/fed/v1)。画面の合言葉ではなく、アカウントの署名で確かめる。
use crate::auth::verify_read;
use crate::node::Node;
use crate::orgs::{accept_join, open_invite, receive, Refusal};
use crate::store::{read_json, write_json};
use agency_proto::org::SignedOrg;
use agency_proto::runtime::RuntimeError;
use agency_proto::{clock, Envelope, Id, JoinRequest, Kind, RuntimeDecision, Signed, TeamView};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

type Shared = State<Arc<Node>>;

fn refuse((status, message): Refusal) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// 400 and 404 from the team runtime are refusals and pass through; everything else is 503, so the sender retries.
fn runtime_refusal(error: RuntimeError) -> Refusal {
    match error.status {
        400 => (StatusCode::BAD_REQUEST, error.message),
        404 => (StatusCode::NOT_FOUND, error.message),
        _ => (StatusCode::SERVICE_UNAVAILABLE, error.message),
    }
}

fn parse_id(raw: &str) -> Result<Id, Refusal> {
    Id::parse(raw).map_err(|_| (StatusCode::NOT_FOUND, "見つからない".into()))
}

fn parse_body<T: DeserializeOwned>(bytes: &Bytes) -> Result<T, Refusal> {
    serde_json::from_slice(bytes).map_err(|_| (StatusCode::BAD_REQUEST, "本文の形が違う".into()))
}

fn definition_of(node: &Node, org: &Id) -> Result<SignedOrg, Refusal> {
    node.data
        .load_org(org)
        .ok_or((StatusCode::NOT_FOUND, "見つからない".into()))
}

/// 組織の定義: 所属する人の署名つきの読み出し、または有効な招待の合言葉で読める
async fn get_definition(
    State(node): Shared,
    Path(org): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let result = (|| {
        let org = parse_id(&org)?;
        let doc = definition_of(&node, &org)?;
        let has_invite = query
            .get("code")
            .is_some_and(|code| open_invite(&node, &org, code).is_some());
        if !has_invite {
            verify_read(&doc.body, &headers, "GET", uri.path())?;
        }
        Ok(doc)
    })();
    result.map_or_else(refuse, |doc| Json(doc).into_response())
}

async fn post_definition(State(node): Shared, Path(org): Path<String>, bytes: Bytes) -> Response {
    let result = async {
        let org = parse_id(&org)?;
        let doc: SignedOrg = parse_body(&bytes)?;
        if doc.body.org != org {
            return Err((StatusCode::BAD_REQUEST, "届け先と組織が違う".into()));
        }
        receive(&node, doc).await
    };
    result.await.map_or_else(refuse, |()| {
        Json(json!({ "accepted": true })).into_response()
    })
}

async fn post_join(State(node): Shared, Path(org): Path<String>, bytes: Bytes) -> Response {
    let result = async {
        let org = parse_id(&org)?;
        let signed: Signed<JoinRequest> = parse_body(&bytes)?;
        accept_join(&node, &org, signed).await
    };
    result
        .await
        .map_or_else(refuse, |doc| Json(doc).into_response())
}

/// The TeamView of a team on this node, only for a signed read by a member of its org.
async fn get_view(
    State(node): Shared,
    Path((org, team)): Path<(String, String)>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let hosted = node
            .config()
            .hosted(&org, &team)
            .cloned()
            .ok_or((StatusCode::NOT_FOUND, "見つからない".into()))?;
        let doc = definition_of(&node, &org)?;
        verify_read(&doc.body, &headers, "GET", uri.path())?;
        node.runtimes
            .view(&hosted)
            .await
            .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.message))
    };
    result
        .await
        .map_or_else(refuse, |view: TeamView| Json(view).into_response())
}

/// 封筒を確かめて当てる。形、届け先、所属、署名、期限、2 度目でないことの順
async fn post_envelope(
    State(node): Shared,
    Path((org, team)): Path<(String, String)>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let (org, team) = (parse_id(&org)?, parse_id(&team)?);
        let signed: Signed<Envelope> = parse_body(&bytes)?;
        let envelope = &signed.body;
        envelope
            .validate()
            .map_err(|message| (StatusCode::BAD_REQUEST, message))?;
        if envelope.org != org || envelope.team != team {
            return Err((
                StatusCode::BAD_REQUEST,
                "封筒の組織とチームが届け先と違う".into(),
            ));
        }
        let hosted = node
            .config()
            .hosted(&org, &team)
            .cloned()
            .ok_or((StatusCode::NOT_FOUND, "見つからない".into()))?;
        let doc = definition_of(&node, &org)?;
        let person = doc
            .body
            .member(&envelope.account)
            .ok_or((StatusCode::NOT_FOUND, "見つからない".into()))?;
        if signed.signer != envelope.account || !signed.verify(Kind::Envelope, &person.public_key) {
            return Err((StatusCode::UNAUTHORIZED, "署名が合わない".into()));
        }
        if !envelope.is_fresh(clock::now()) {
            return Err((
                StatusCode::BAD_REQUEST,
                "封筒の期限が切れているか、時刻が先すぎる".into(),
            ));
        }
        let _guard = node.writes.lock().await;
        let applied_path = node.data.dir("applied.json");
        let mut applied: BTreeMap<String, String> = read_json(&applied_path).unwrap_or_default();
        if applied.contains_key(envelope.id.as_str()) {
            return Ok(json!({ "duplicate": true }));
        }
        let decision = RuntimeDecision {
            kind: envelope.kind,
            target: envelope.target.clone(),
            by: person.id.clone(),
            payload: envelope.payload.clone(),
        };
        node.runtimes
            .decide(&hosted, decision)
            .await
            .map_err(runtime_refusal)?;
        applied.insert(envelope.id.as_str().to_owned(), clock::format(clock::now()));
        write_json(&applied_path, &applied)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        Ok(json!({ "applied": true }))
    };
    result
        .await
        .map_or_else(refuse, |body| Json(body).into_response())
}

pub fn router(node: Arc<Node>) -> Router {
    Router::new()
        .route(
            "/fed/v1/orgs/{org}/definition",
            get(get_definition).post(post_definition),
        )
        .route("/fed/v1/orgs/{org}/join", post(post_join))
        .route("/fed/v1/teams/{org}/{team}/view", get(get_view))
        .route("/fed/v1/teams/{org}/{team}/envelopes", post(post_envelope))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(node)
}
