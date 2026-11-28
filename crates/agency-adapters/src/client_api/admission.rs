//! Who a request is let in as (docs/protocol.md, "Who is let in"): the direct local request, the owner's
//! sign-in, a guest, or a claimant, and what each may write. `guard` runs before every route behind it.
use super::{claimants, guests, header_text, refuse, Admitted, ClientApi, Shared, TOKEN_HEADER};
use crate::access::Identity;
use agency_core::clients_seen;
use agency_core::guests::Caller;
use agency_proto::client_version;
use agency_proto::clock::Duration;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::net::SocketAddr;
use std::sync::PoisonError;

/// Whether the request came through Cloudflare (its tunnel or proxy adds these headers).
pub fn is_via_cloudflare(headers: &HeaderMap) -> bool {
    headers.contains_key("cf-ray") || headers.contains_key("cf-connecting-ip")
}

async fn admit(
    api: &ClientApi,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<Admitted, (StatusCode, &'static str)> {
    let host = header_text(headers, header::HOST).unwrap_or("");
    let is_public = api
        .access
        .as_ref()
        .is_some_and(|access| access.provider().public_host() == host);
    if !is_via_cloudflare(headers) && !is_public {
        let is_local_host =
            host == format!("127.0.0.1:{}", api.port) || host == format!("localhost:{}", api.port);
        return match (peer.ip().is_loopback(), is_local_host) {
            // On a machine other accounts log in to, each of them reaches the loopback address too.
            (true, true) if !api.node.config().local_clients => {
                Err((StatusCode::FORBIDDEN, "this node admits no local clients"))
            }
            (true, true) => Ok(Admitted::Local),
            (true, false) => Err((StatusCode::MISDIRECTED_REQUEST, "wrong host")),
            (false, _) => Err((StatusCode::FORBIDDEN, "the Client API is local only")),
        };
    }
    let access = api
        .access
        .as_ref()
        .ok_or((StatusCode::FORBIDDEN, "this node has no sign-in setting"))?;
    match access.verify(access.provider().token(headers)).await {
        // Only a known identity is admitted: the owner's email, which always wins, or a current guest's.
        // Any other sign-in, one without an email included, is refused.
        Ok(identity) if is_owner(api, &identity) => Ok(Admitted::Access(identity)),
        Ok(identity) => identity
            .email
            .as_deref()
            .and_then(|email| admitted_as_other(api, email))
            .ok_or_else(|| {
                let why = if identity.has_unverified_email {
                    "whose email the provider did not say it verified, so it names no guest"
                } else {
                    "that is neither the owner, a guest, nor a claim link's address"
                };
                (api.log)(&format!(
                    "refused an {} sign-in {why}",
                    access.provider().name()
                ));
                (
                    StatusCode::FORBIDDEN,
                    "this sign-in is not the node's owner or a guest of it",
                )
            }),
        Err(reason) => {
            let name = access.provider().name();
            (api.log)(&format!("refused an {name} request ({reason})"));
            Err((
                StatusCode::FORBIDDEN,
                if access.provider().access_config().is_some() {
                    "not signed in through Cloudflare Access"
                } else {
                    "not signed in through the node's OpenID Connect provider"
                },
            ))
        }
    }
}

/// What an Access sign-in that is not the owner's is admitted as: a current guest, else a person whose address a
/// claim link was made for (and no more than that), else nothing.
fn admitted_as_other(api: &ClientApi, email: &str) -> Option<Admitted> {
    if let Some(guest) = agency_core::guests::admit(&api.node, email) {
        return Some(Admitted::Guest(guest));
    }
    agency_core::claims::is_claimant(&api.node, email)
        .then(|| Admitted::Claimant(email.trim().to_lowercase()))
}

/// Constant-time comparison, so the token cannot be guessed byte by byte from response times.
pub(crate) fn is_same_secret(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// A write needs the page's token, a JSON body (a DELETE has none), and the owner: on a node with an
/// access setting, the owner signed in through Access, even on loopback.
fn check_write(
    api: &ClientApi,
    admitted: &Admitted,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, &'static str)> {
    let has_token =
        header_text(headers, TOKEN_HEADER).is_some_and(|token| is_same_secret(token, &api.token));
    if !has_token {
        return Err((StatusCode::FORBIDDEN, "missing or wrong write token"));
    }
    let is_json = header_text(headers, header::CONTENT_TYPE)
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim() == "application/json");
    if !is_json && method != Method::DELETE {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "the body must be application/json",
        ));
    }
    match admitted {
        // With a sign-in setting (`access` or `oidc`) the owner is who the provider says it is, so a process
        // of the same OS user on this machine cannot write as the owner. Checked against the setting itself,
        // so a malformed one (which disables the provider) does not make loopback the owner again.
        Admitted::Local if api.node.config().has_sign_in() => Err((
            StatusCode::FORBIDDEN,
            if api.node.config().access.is_some() {
                "this node is behind Access: writes come through its public host name, signed in as the owner"
            } else {
                "this node is behind its OpenID Connect provider: writes come through its public host name, signed in as the owner"
            },
        )),
        Admitted::Local => Ok(()),
        Admitted::Access(identity) if is_owner(api, identity) => Ok(()),
        Admitted::Access(_) => Err((StatusCode::FORBIDDEN, "only the node's owner may decide")),
        // A guest never writes as the owner: only the writes of its own table, scoped to its member there.
        Admitted::Guest(guest) if guests::may_write(guest, method, path) => Ok(()),
        Admitted::Guest(_) => Err((StatusCode::FORBIDDEN, "a guest may not do this")),
        // A claimant never writes as anyone: only the claim, in its own arm.
        Admitted::Claimant(_) if claimants::may_write(method, path) => Ok(()),
        Admitted::Claimant(_) => Err((StatusCode::FORBIDDEN, "a claimant may not do this")),
    }
}

/// Whether the sign-in is the configured owner's.
pub(crate) fn is_owner(api: &ClientApi, identity: &Identity) -> bool {
    api.access
        .as_ref()
        .is_some_and(|access| access.provider().is_owner(identity))
}

pub(super) async fn guard(
    State(api): Shared,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    let admitted = match admit(&api, peer, request.headers()).await {
        Ok(admitted) => admitted,
        Err((status, message)) => return refuse(status, message),
    };
    let is_write = !matches!(*request.method(), Method::GET | Method::HEAD);
    // A guest's reads are checked here against its table; its writes by its own arm of `check_write`.
    if let Admitted::Guest(guest) = &admitted {
        if !is_write && !guests::may(guest, request.method(), request.uri().path()) {
            return refuse(StatusCode::FORBIDDEN, "a guest may not do this");
        }
    }
    if matches!(admitted, Admitted::Claimant(_))
        && !is_write
        && !claimants::may(request.method(), request.uri().path())
    {
        return refuse(StatusCode::FORBIDDEN, "a claimant may not do this");
    }
    if is_write {
        let path = request.uri().path().to_owned();
        if let Err((status, message)) =
            check_write(&api, &admitted, request.method(), &path, request.headers())
        {
            return refuse(status, message);
        }
    }
    note_client(&api, request.headers());
    request.extensions_mut().insert(admitted);
    next.run(request).await
}

/// Notes the app that made an admitted request, from its `x-agency-client` header: at most once an hour for
/// each version, so a client that calls often costs no write. A header that does not read is ignored; a
/// failure to note it never fails the request.
fn note_client(api: &ClientApi, headers: &HeaderMap) {
    let Some(seen) = header_text(headers, client_version::HEADER).and_then(client_version::parse)
    else {
        return;
    };
    let now = api.node.now();
    {
        let mut noted = api
            .client_notes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let key = (seen.platform, seen.version.clone());
        if noted.get(&key).is_some_and(|at| now - *at < NOTE_EVERY) {
            return;
        }
        if noted.len() >= NOTED_KEPT {
            noted.clear();
        }
        noted.insert(key, now);
    }
    if let Err(error) = clients_seen::note(&api.node, &seen) {
        (api.log)(&format!("could not record an app version ({error})"));
    }
}

/// How long a version is left before it is noted again, and how many versions the throttle remembers.
const NOTE_EVERY: Duration = Duration::hours(1);
const NOTED_KEPT: usize = 256;

/// Who the request acts as: a guest as its hosted member, anyone else admitted as the node's own account.
pub(super) fn caller_of(admitted: &Admitted) -> Caller {
    match admitted {
        Admitted::Guest(guest) => Caller::Guest(guest.clone()),
        Admitted::Local | Admitted::Access(_) => Caller::Owner,
        // The guard lets a claimant reach only the claim routes, which do not ask who the caller is; one that got
        // here would otherwise be taken for the owner.
        Admitted::Claimant(_) => unreachable!("a claimant reaches only the claim routes"),
    }
}

/// Whether the request is the node's owner's: a direct local one (the machine's own user), or one
/// through Access signed in as the configured owner. Other Access sign-ins may read this node's data
/// but are not told about its machine.
pub(super) fn is_owner_request(api: &ClientApi, admitted: &Admitted) -> bool {
    match admitted {
        Admitted::Local => true,
        Admitted::Access(identity) => is_owner(api, identity),
        Admitted::Guest(_) | Admitted::Claimant(_) => false,
    }
}
