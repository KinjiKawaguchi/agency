//! Invites and joins (kind `join`). The root key's node creates a single-use invite that expires after
//! seven days; the invited node checks the root key's fingerprint, then sends a signed join request.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{is_root, load, publish, sign};
use crate::ports::DefinitionAccess;
use crate::routes;
use agency_proto::clock::{self, Duration};
use agency_proto::invite::{code_digest, code_from_bytes, Invite};
use agency_proto::org::{verify_successor, SignedOrg};
use agency_proto::{Id, JoinRequest, Kind, OrgDefinition, Person, Role, Signed};
use serde::{Deserialize, Serialize};

pub const INVITE_DAYS: i64 = 7;
/// How far a join request's time may be from the root node's clock, either way.
pub const JOIN_SKEW_SECONDS: i64 = 600;

/// What an invite grants. Stored under the SHA-256 of its code; the code itself is never stored.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteRecord {
    pub org: Id,
    pub department: Option<Id>,
    pub role: Role,
    pub expires_at: String,
    pub used_by: Option<Id>,
}

/// On the root key's node: creates an invite and returns its link.
pub fn invite(node: &Node, org: &Id, department: Option<Id>, role: Role) -> Result<String> {
    let current = load(node, org)?;
    if !is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("only the node with the root key of org {org} can invite"),
        ));
    }
    let code = code_from_bytes(node.random());
    let expires_at = clock::format(node.now() + Duration::days(INVITE_DAYS));
    let record = InviteRecord {
        org: org.clone(),
        department,
        role,
        expires_at,
        used_by: None,
    };
    node.store().save_invite(&code_digest(&code), &record)?;
    Ok(Invite {
        node: node.url(),
        org: org.clone(),
        fp: current.body.root.public_key.fingerprint(),
        code,
    }
    .to_link())
}

/// The invite, if it is for this org, unused, and not expired.
pub fn open_invite(node: &Node, org: &Id, code: &str) -> Option<InviteRecord> {
    let record = node.store().load_invite(&code_digest(code))?;
    let is_open = &record.org == org
        && record.used_by.is_none()
        && clock::parse(&record.expires_at).is_some_and(|at| at > node.now());
    is_open.then_some(record)
}

/// On the root key's node: checks the join request's signature, the invite, and duplicates, then signs
/// the next version with the new person added and returns it.
pub async fn accept_join(node: &Node, org: &Id, signed: Signed<JoinRequest>) -> Result<SignedOrg> {
    let _guard = node.write_lock().lock().await;
    let request = &signed.body;
    if &request.org != org
        || signed.signer != request.account
        || !signed.verify(Kind::Join, &request.public_key)
    {
        return Err(Error::unauthorized(
            "the join request's signature does not match",
        ));
    }
    let is_label_valid = (1..=80).contains(&request.label.chars().count());
    let is_fresh = clock::parse(&request.at)
        .is_some_and(|at| (node.now() - at).whole_seconds().abs() <= JOIN_SKEW_SECONDS);
    if !is_label_valid || !is_fresh {
        return Err(Error::bad_request("malformed name or time"));
    }
    let record = open_invite(node, org, &request.code).ok_or_else(|| {
        Error::new(
            ErrorKind::Forbidden,
            "no such invite, or it expired or was used",
        )
    })?;
    let current = load(node, org)?;
    if current
        .body
        .people
        .iter()
        .any(|one| one.account == request.account || one.id == request.account)
    {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that account is already in the org",
        ));
    }
    let person = Person {
        id: request.account.clone(),
        label: request.label.clone(),
        account: request.account.clone(),
        public_key: request.public_key.clone(),
        department: record.department.clone(),
        role: record.role,
        devices: vec![],
    };
    let mut body = OrgDefinition::successor_of(&current, node.now_text());
    body.people.push(person);
    let doc = sign(node, body)?;
    publish(node, &doc)?;
    node.store().save_invite(
        &code_digest(&request.code),
        &InviteRecord {
            used_by: Some(request.account.clone()),
            ..record
        },
    )?;
    Ok(doc)
}

/// Joins an org from an invite link. Stops unless the org's root key matches the fingerprint in the link,
/// and unless `confirm` agrees after seeing the definition.
pub async fn join(
    node: &Node,
    link: &str,
    confirm: impl FnOnce(&OrgDefinition) -> bool,
) -> Result<SignedOrg> {
    let invite = Invite::parse(link).map_err(Error::bad_request)?;
    let access = DefinitionAccess::InviteCode(invite.code.clone());
    let current = node
        .peers()
        .get_definition(&invite.node, &routes::definition(&invite.org), &access)
        .await
        .map_err(|error| {
            Error::new(
                ErrorKind::Unavailable,
                format!("the inviting node did not return the org definition: {error}"),
            )
        })?;
    let root = &current.body.root;
    let is_genuine = current.body.org == invite.org
        && root.public_key.fingerprint() == invite.fp
        && current.signer == root.account
        && current.verify(Kind::Org, &root.public_key);
    if !is_genuine {
        return Err(Error::unauthorized("the org's root key does not match the fingerprint in the link (the node may be fake); not joining"));
    }
    if !confirm(&current.body) {
        return Err(Error::bad_request("joining was cancelled"));
    }
    let config = node.config();
    let request = JoinRequest {
        org: invite.org.clone(),
        code: invite.code.clone(),
        account: config.account.id.clone(),
        label: config.account.label.clone(),
        public_key: node.key().public(),
        at: node.now_text(),
    };
    let signed = Signed::sign(Kind::Join, request, config.account.id.clone(), node.key())
        .map_err(|error| Error::internal(error.to_string()))?;
    let joined = node
        .peers()
        .post_join(&invite.node, &routes::join(&invite.org), &signed)
        .await
        .map_err(|error| {
            Error::new(
                ErrorKind::Forbidden,
                format!("the join was refused: {error}"),
            )
        })?;
    verify_successor(&current, &joined).map_err(Error::bad_request)?;
    if joined.body.member(&config.account.id).is_none() {
        return Err(Error::bad_request(
            "the returned org definition does not include this account",
        ));
    }
    node.store().save_org(&joined)?;
    node.update_config(|config| {
        config.trust_with_root_node(invite.org.clone(), invite.fp.clone(), invite.node.clone())
    })?;
    Ok(joined)
}
