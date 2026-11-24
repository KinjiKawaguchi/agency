//! Org definitions. The node with the root key signs each new version and sends it, through the outbox,
//! to the nodes of the org's teams. Other nodes accept only the next version of the one they have, or a
//! first copy signed by a root key they trust.
use crate::error::{Error, ErrorKind, Result};
use crate::invites::open_invite;
use crate::node::Node;
use crate::outbox::{enqueue, Parcel, ParcelBody};
use crate::read_auth::{verify_read, ReadAuth};
use agency_proto::org::{verify_genesis, verify_successor, Root, SignedOrg, TeamPlace};
use agency_proto::{Id, Kind, OrgDefinition, Person, Role, Signed};

pub(crate) fn load(node: &Node, org: &Id) -> Result<SignedOrg> {
    node.store()
        .load_org(org)
        .ok_or_else(|| Error::not_found(format!("no definition of org {org}")))
}

pub(crate) fn sign(node: &Node, body: OrgDefinition) -> Result<SignedOrg> {
    body.validate().map_err(Error::bad_request)?;
    Signed::sign(Kind::Org, body, node.account(), node.key())
        .map_err(|error| Error::internal(error.to_string()))
}

/// Whether this node holds the org's root key.
pub fn is_root(node: &Node, doc: &SignedOrg) -> bool {
    doc.body.root.account == node.account() && doc.body.root.public_key == node.key().public()
}

/// Saves a new version and queues it for every other node that hosts one of the org's teams.
pub(crate) fn publish(node: &Node, doc: &SignedOrg) -> Result<()> {
    node.store().save_org(doc)?;
    let me = node.url();
    let mut targets: Vec<String> = doc
        .body
        .teams
        .iter()
        .map(|team| team.node.clone())
        .filter(|url| url != &me)
        .collect();
    targets.sort();
    targets.dedup();
    let now = node.now_text();
    for to in targets {
        enqueue(
            node,
            &Parcel::new(to, ParcelBody::Org(Box::new(doc.clone())), &now),
        )?;
    }
    Ok(())
}

/// Creates an org with this node's account as its root key and only member.
pub fn create(node: &Node, org: Id, label: String) -> Result<SignedOrg> {
    if node.store().load_org(&org).is_some() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("org {org} already exists"),
        ));
    }
    let config = node.config();
    let account = config.account.id.clone();
    let public_key = node.key().public();
    let me = Person {
        id: account.clone(),
        label: config.account.label,
        account: account.clone(),
        public_key: public_key.clone(),
        department: None,
        role: Role::Member,
    };
    let root = Root {
        account,
        public_key: public_key.clone(),
    };
    let body = OrgDefinition {
        org: org.clone(),
        label,
        version: 1,
        prev: None,
        root,
        departments: vec![],
        people: vec![me],
        teams: vec![],
        revoked: vec![],
        at: node.now_text(),
    };
    let doc = sign(node, body)?;
    node.store().save_org(&doc)?;
    node.update_config(|config| config.trust(org, public_key.fingerprint()))?;
    Ok(doc)
}

/// On the root key's node: edits the definition into its next version, signs it, and sends it out.
pub fn amend(
    node: &Node,
    org: &Id,
    edit: impl FnOnce(&mut OrgDefinition) -> Result<()>,
) -> Result<SignedOrg> {
    let current = load(node, org)?;
    if !is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("this node does not hold the root key of org {org}"),
        ));
    }
    let mut body = OrgDefinition::successor_of(&current, node.now_text());
    edit(&mut body)?;
    let doc = sign(node, body)?;
    publish(node, &doc)?;
    Ok(doc)
}

pub fn add_team(node: &Node, org: &Id, team: TeamPlace) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if body.team(&team.id).is_some() {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("team {} already exists", team.id),
            ));
        }
        body.teams.push(team);
        Ok(())
    })
}

/// The org definition for a member's signed read, or for anyone with an open invite code.
pub fn read_definition(
    node: &Node,
    org: &Id,
    code: Option<&str>,
    auth: &ReadAuth,
    path: &str,
) -> Result<SignedOrg> {
    let doc = load(node, org).map_err(|_| Error::not_found("not found"))?;
    let has_invite = code.is_some_and(|code| open_invite(node, org, code).is_some());
    if !has_invite {
        verify_read(&doc.body, auth, "GET", path, node.now())?;
    }
    Ok(doc)
}

/// Accepts a definition sent by another node: with a copy, only its next version; without one, only a
/// version signed by the root key whose fingerprint this node trusts.
pub async fn receive(node: &Node, doc: SignedOrg) -> Result<()> {
    let _guard = node.write_lock().lock().await;
    let org = doc.body.org.clone();
    let Some(current) = node.store().load_org(&org) else {
        return receive_first(node, doc);
    };
    if current.body.version >= doc.body.version {
        let is_same = current.body.version == doc.body.version && current.digest() == doc.digest();
        return if is_same {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Conflict,
                format!("this node has version {} of {org}", current.body.version),
            ))
        };
    }
    verify_successor(&current, &doc).map_err(Error::bad_request)?;
    node.store().save_org(&doc)
}

fn receive_first(node: &Node, doc: SignedOrg) -> Result<()> {
    let org = &doc.body.org;
    let trusted = node
        .config()
        .trusted_fingerprint(org)
        .map(str::to_owned)
        .ok_or_else(|| Error::not_found("this node does not trust that org"))?;
    let root = &doc.body.root;
    let is_trusted = root.public_key.fingerprint() == trusted
        && doc.signer == root.account
        && doc.verify(Kind::Org, &root.public_key);
    if !is_trusted {
        return Err(Error::unauthorized("not signed by the trusted root key"));
    }
    if doc.body.version == 1 {
        verify_genesis(&doc).map_err(Error::bad_request)?;
    }
    doc.body.validate().map_err(Error::bad_request)?;
    node.store().save_org(&doc)
}
