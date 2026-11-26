//! Org definitions. The node with the root key signs each new version and sends it, through the outbox,
//! to the nodes of the org's teams. Other nodes accept only the next version of the one they have, or a
//! first copy signed by a root key they trust.
use crate::error::{Error, ErrorKind, Result};
use crate::invites::open_invite;
use crate::node::Node;
use crate::outbox::{enqueue, Parcel, ParcelBody};
use crate::read_auth::{sign_read, verify_read_once, ReadAuth};
use crate::routes;
use crate::unseen;
use agency_proto::clock::{self, Duration, OffsetDateTime};
use agency_proto::org::{
    verify_genesis, verify_successor, Root, SignedOrg, Succession, TeamPlace, Versions,
    RECOVERY_WAIT_SECONDS,
};
use agency_proto::{Id, Kind, OrgDefinition, Person, PublicKey, Role, Signed};

pub(crate) fn load(node: &Node, org: &Id) -> Result<SignedOrg> {
    node.store()
        .load_org(org)
        .ok_or_else(|| Error::not_found(format!("no definition of org {org}")))
}

/// Signs a version with the root key it names: version 1, or any next version that keeps the root.
pub(crate) fn sign(node: &Node, body: OrgDefinition) -> Result<SignedOrg> {
    let key = body.root.public_key.clone();
    sign_as(node, body, &key)
}

/// Signs a version with `key`, a root key this node holds: the org's own root key (`RootKeys`), or the
/// account key for an org made before root keys were kept apart. A rotation signs the version that
/// names the new root with the current one.
fn sign_as(node: &Node, body: OrgDefinition, key: &PublicKey) -> Result<SignedOrg> {
    // A version the root signs never carries a waiting recovery, so signing any version cancels it.
    let body = OrgDefinition {
        root: Root {
            pending: None,
            ..body.root
        },
        ..body
    };
    body.validate().map_err(Error::bad_request)?;
    let signer = node.account();
    if key == &node.key().public() {
        return Signed::sign(Kind::Org, body, signer, node.key())
            .map_err(|error| Error::internal(error.to_string()));
    }
    let (org, purpose) = (
        body.org.clone(),
        format!("Sign version {} of {}", body.version, body.label),
    );
    let signed = Signed::sign_with(Kind::Org, body, signer, |text| {
        node.root_keys().sign(&org, key, text, &purpose)
    })
    .map_err(|error| Error::internal(error.to_string()))?;
    signed.map_err(|why| {
        Error::new(
            ErrorKind::Forbidden,
            format!("the root key of {org} did not sign: {why}"),
        )
    })
}

/// Whether this node holds the org's root key: its own root key, or the account key for an org made
/// before root keys were kept apart.
pub fn is_root(node: &Node, doc: &SignedOrg) -> bool {
    let root = &doc.body.root;
    root.account == node.account()
        && (root.public_key == node.key().public()
            || node.root_keys().holds(&doc.body.org, &root.public_key))
}

/// Saves a new version and queues it for every other node that hosts one of the org's teams. Teams that
/// run here are skipped even if their place names another origin.
pub(crate) fn publish(node: &Node, doc: &SignedOrg) -> Result<()> {
    node.store().save_org(doc)?;
    let me = node.url();
    let targets = doc
        .body
        .teams
        .iter()
        .filter(|team| !node.runs_here(&doc.body.org, &team.id))
        .map(|team| team.node.clone())
        .filter(|url| url != &me)
        .collect();
    queue_for(node, doc, targets)
}

/// Queues `doc` for each of the nodes, once each.
fn queue_for(node: &Node, doc: &SignedOrg, mut targets: Vec<String>) -> Result<()> {
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

/// The teams `previous` lists and `next` does not.
fn removed_teams<'a>(previous: &'a OrgDefinition, next: &OrgDefinition) -> Vec<&'a TeamPlace> {
    let is_listed = |team: &&TeamPlace| next.team(&team.id).is_some();
    previous
        .teams
        .iter()
        .filter(|team| !is_listed(team))
        .collect()
}

/// Queues the version that removes a team for that team's own node as well. Versions go to the nodes of
/// the teams still listed, so without this the node of a removed team would not hear it was removed.
fn tell_removed(node: &Node, previous: &OrgDefinition, doc: &SignedOrg) -> Result<()> {
    let me = node.url();
    let kept: Vec<&String> = doc.body.teams.iter().map(|team| &team.node).collect();
    let targets = removed_teams(previous, &doc.body)
        .into_iter()
        .map(|team| team.node.clone())
        .filter(|url| url != &me && !kept.contains(&url))
        .collect();
    queue_for(node, doc, targets)
}

/// After a version that names a new root (signed by the old one, as `verify_successor` checked), the
/// trust this node keeps for the org moves to the new root, so a later first copy is checked against it.
/// A root key this node made for a recovery that was then cancelled will sign nothing: it is forgotten.
fn forget_cancelled_root(node: &Node, previous: &SignedOrg, next: &SignedOrg) {
    let made = previous
        .body
        .root
        .pending
        .as_ref()
        .and_then(|one| one.public_key.as_ref());
    let Some(made) = made else { return };
    let is_cancelled = next.body.root.pending.is_none() && &next.body.root.public_key != made;
    if is_cancelled && node.root_keys().holds(&next.body.org, made) {
        let _ = node.root_keys().forget(&next.body.org, made);
    }
}

fn follow_new_root(node: &Node, previous: &SignedOrg, next: &SignedOrg) -> Result<()> {
    let (old, new) = (&previous.body.root.public_key, &next.body.root.public_key);
    if old == new {
        return Ok(());
    }
    let (org, from, to) = (&next.body.org, old.fingerprint(), new.fingerprint());
    node.update_config(|config| config.retrust(org, &from, to))
}

/// A team the org removed has no view to keep: this node's replica of it is deleted.
fn forget_removed(node: &Node, previous: &OrgDefinition, next: &OrgDefinition) -> Result<()> {
    for team in removed_teams(previous, next) {
        node.store().delete_replica(&next.org, &team.id)?;
    }
    Ok(())
}

/// Creates an org with this node's account as its root key and only member.
pub fn create(node: &Node, org: Id, label: String) -> Result<SignedOrg> {
    let is_known = node.config().trusted_fingerprint(&org).is_some();
    if is_known || node.store().load_org(&org).is_some() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("org {org} already exists"),
        ));
    }
    let config = node.config();
    let account = config.account.id.clone();
    let root_key = node
        .root_keys()
        .create(&org)
        .map_err(|why| Error::internal(format!("cannot make a root key for {org}: {why}")))?;
    let me = Person {
        id: account.clone(),
        label: config.account.label,
        account: account.clone(),
        public_key: node.key().public(),
        department: None,
        role: Role::Member,
        devices: vec![],
    };
    let root = Root::new(account, root_key.clone());
    let body = OrgDefinition {
        kpis: vec![],
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
    node.update_config(|config| config.trust(org, root_key.fingerprint()))?;
    Ok(doc)
}

/// Moves an org to a new root key: the next version names it and is signed by the current root key,
/// and every later version by the new one. The new key is made here (`RootKeys`); the old one is
/// forgotten once the version naming the new one is published. Other nodes check it as any next
/// version (`verify_successor`: signed by the root of the version they have).
pub fn rotate_root(node: &Node, org: &Id) -> Result<SignedOrg> {
    let current = load(node, org)?;
    if !is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("this node does not hold the root key of org {org}"),
        ));
    }
    let old = current.body.root.public_key.clone();
    let new = node
        .root_keys()
        .create(org)
        .map_err(|why| Error::internal(format!("cannot make a root key for {org}: {why}")))?;
    let mut body = OrgDefinition::successor_of(&current, node.now_text());
    body.root.public_key = new.clone();
    let doc = match sign_as(node, body, &old) {
        Ok(doc) => doc,
        Err(error) => {
            let _ = node.root_keys().forget(org, &new);
            return Err(error);
        }
    };
    publish(node, &doc)?;
    let (old_fingerprint, new_fingerprint) = (old.fingerprint(), new.fingerprint());
    node.update_config(|config| config.retrust(org, &old_fingerprint, new_fingerprint))?;
    if old != node.key().public() {
        node.root_keys()
            .forget(org, &old)
            .map_err(|why| Error::internal(format!("the old root key stays: {why}")))?;
    }
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
    forget_cancelled_root(node, &current, &doc);
    tell_removed(node, &current.body, &doc)?;
    forget_removed(node, &current.body, &doc.body)?;
    Ok(doc)
}

pub fn add_team(node: &Node, org: &Id, team: TeamPlace) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if let Some(department) = &team.department {
            if !body.departments.iter().any(|one| &one.id == department) {
                return Err(Error::not_found(format!(
                    "no department {department} in org {org}"
                )));
            }
        }
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

/// Moves a team's place to another node origin: the next version, changed only there. A malformed origin
/// fails the definition's own check (`validate`). Other nodes route
/// to the team by its place, so after this node's URL changes, its teams are moved to the new origin.
pub fn move_team(node: &Node, org: &Id, team: &Id, origin: String) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let place = body
            .teams
            .iter_mut()
            .find(|place| &place.id == team)
            .ok_or_else(|| Error::not_found(format!("team {team} is not in org {org}")))?;
        if place.node == origin {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("team {team} is already at {origin}"),
            ));
        }
        place.node = origin;
        Ok(())
    })
}

/// A team that runs on this node, while its org places it elsewhere (or not at all). It still runs and
/// is served here, but other nodes look for it at `place`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Misplaced {
    pub org: Id,
    pub team: Id,
    pub place: Option<String>,
}

/// The hosted teams whose place in their org is not this node's URL.
pub fn misplaced_teams(node: &Node) -> Vec<Misplaced> {
    let config = node.config();
    let misplaced = config.teams.iter().filter_map(|hosted| {
        let doc = node.store().load_org(&hosted.org);
        let place = doc
            .as_ref()
            .and_then(|doc| doc.body.team(&hosted.team))
            .map(|place| place.node.clone());
        (place.as_deref() != Some(config.url.as_str())).then(|| Misplaced {
            org: hosted.org.clone(),
            team: hosted.team.clone(),
            place,
        })
    });
    misplaced.collect()
}

/// The org definition for a member's signed read, or for anyone with an open invite code.
pub fn read_definition(
    node: &Node,
    org: &Id,
    code: Option<&str>,
    auth: &ReadAuth,
    path: &str,
) -> Result<SignedOrg> {
    let doc = node.store().load_org(org);
    let has_invite = code.is_some_and(|code| open_invite(node, org, code).is_some());
    if !has_invite {
        let definition = doc.as_ref().map(|doc| &doc.body);
        verify_read_once(node, definition, doc.is_some(), auth, "GET", path)?;
    }
    doc.ok_or_else(|| unseen::refused("an invite code for an org this node does not have"))
}

/// Accepts a definition sent by another node: with a copy, only its next version; without one, only a
/// version signed by the root key whose fingerprint this node trusts. A definition not signed by the
/// root key this node knows for the org (the current one, or the trusted one) is refused like an
/// unknown org (see `unseen`), so only the root's node learns which version this node has.
pub async fn receive(node: &Node, doc: SignedOrg) -> Result<()> {
    let _guard = node.write_lock().lock().await;
    let org = doc.body.org.clone();
    let Some(current) = node.store().load_org(&org) else {
        return receive_first(node, doc);
    };
    // A version this node already took, byte for byte, is answered as taken. It is checked by its
    // digest against this node's own history before any signature: a version that named a new root is
    // signed by the old one, which is no longer the root of the copy here.
    let kept = node.store().load_org_version(&org, doc.body.version);
    if kept.is_some_and(|kept| kept.digest() == doc.digest()) {
        return Ok(());
    }
    let root = &current.body.root;
    let key = (doc.signer == root.account).then_some(&root.public_key);
    let recovery = root
        .recovery
        .as_ref()
        .filter(|_| doc.signer == root.account);
    let is_by_recovery = || recovery.is_some_and(|key| doc.verify(Kind::Org, key));
    if !unseen::check(&doc, Kind::Org, key) && !is_by_recovery() {
        return Err(unseen::refused(format!(
            "a definition of {org} signed as {}, not with the root key of this node's copy",
            doc.signer
        )));
    }
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
    let succession = verify_successor(&current, &doc).map_err(Error::bad_request)?;
    match succession {
        Succession::Root => {}
        Succession::RecoveryStart => {
            // Noted before the version is kept: a start kept without its note could never finish.
            let now = clock::format(node.now());
            node.store()
                .note_recovery_start(&org, doc.body.version, &now)?;
        }
        Succession::RecoveryFinish => refuse_an_early_finish(node, &current)?,
    }
    // The trust moves first: if keeping the version then fails, a retry takes it again, and the trust
    // already names its root; the other order could keep the version with the trust left behind.
    follow_new_root(node, &current, &doc)?;
    node.store().save_org(&doc)?;
    forget_cancelled_root(node, &current, &doc);
    forget_removed(node, &current.body, &doc.body)
}

/// When a recovery started by `start` takes effect on this node: `RECOVERY_WAIT_SECONDS` after this
/// node took the start, by its own clock.
pub fn recovery_takes_effect(node: &Node, start: &SignedOrg) -> Option<OffsetDateTime> {
    let org = &start.body.org;
    let took = node.store().recovery_started(org, start.body.version)?;
    Some(clock::parse(&took)? + Duration::seconds(RECOVERY_WAIT_SECONDS))
}

fn refuse_an_early_finish(node: &Node, start: &SignedOrg) -> Result<()> {
    let org = &start.body.org;
    match recovery_takes_effect(node, start) {
        Some(at) if at <= node.now() => Ok(()),
        Some(at) => Err(Error::new(
            ErrorKind::Conflict,
            format!(
                "the recovery of {org} takes effect on this node at {}",
                clock::format(at)
            ),
        )),
        None => Err(Error::new(
            ErrorKind::Conflict,
            format!("this node has no record of when the recovery of {org} started"),
        )),
    }
}

fn receive_first(node: &Node, doc: SignedOrg) -> Result<()> {
    let root = &doc.body.root;
    let trusted = node
        .config()
        .trusted_fingerprint(&doc.body.org)
        .map(str::to_owned);
    let is_trusted_root =
        trusted == Some(root.public_key.fingerprint()) && doc.signer == root.account;
    if !unseen::check(&doc, Kind::Org, is_trusted_root.then_some(&root.public_key)) {
        let org = &doc.body.org;
        return Err(unseen::refused(match trusted {
            None => format!("a first definition of {org}, an org this node does not trust"),
            Some(_) => {
                format!("a first definition of {org} not signed by the root key this node trusts")
            }
        }));
    }
    if doc.body.version == 1 {
        verify_genesis(&doc).map_err(Error::bad_request)?;
    }
    doc.body.validate().map_err(Error::bad_request)?;
    node.store().save_org(&doc)
}

/// At most this many versions go in one answer of the versions route; a node further behind asks again.
pub const VERSIONS_PER_ANSWER: u64 = 20;

/// The versions of an org's definition after `after`, oldest first and at most `VERSIONS_PER_ANSWER`,
/// for a member's signed read of `path` (`routes::versions`). They come from this node's history, so a
/// node that joined later than `after` does not have them all and says so.
pub fn read_versions(
    node: &Node,
    org: &Id,
    after: u64,
    auth: &ReadAuth,
    path: &str,
) -> Result<Versions> {
    let doc = node.store().load_org(org);
    let definition = doc.as_ref().map(|doc| &doc.body);
    verify_read_once(node, definition, doc.is_some(), auth, "GET", path)?;
    let current = doc
        .ok_or_else(|| unseen::refused("a read of the versions of an org this node does not have"))?
        .body
        .version;
    let last = current.min(after.saturating_add(VERSIONS_PER_ANSWER));
    let versions = (after.saturating_add(1)..=last)
        .map(|version| node.store().load_org_version(org, version))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            Error::not_found(format!(
                "this node does not have every version of {org} after {after}"
            ))
        })?;
    Ok(Versions { versions })
}

/// Brings this node's copy of an org up to `target` from the node at `origin`, one version at a time:
/// each must be the next one, linked to the one before and signed by its root (`receive`). A node that
/// missed versions (it was offline, or it reads rarely) otherwise could never take a newer one, since
/// it accepts only the next version of the one it has.
pub async fn catch_up(node: &Node, origin: &str, org: &Id, target: u64) -> Result<()> {
    loop {
        let current = load(node, org)?.body.version;
        if current + 1 >= target {
            return Ok(());
        }
        let path = routes::versions(org, current);
        let auth = sign_read(node.key(), &node.account(), "GET", &path, node.now());
        let fetched = node
            .peers()
            .get_versions(origin, &path, &auth)
            .await
            .map_err(|error| Error::new(ErrorKind::Unavailable, error.to_string()))?;
        if fetched.versions.is_empty() {
            return Err(Error::new(
                ErrorKind::Unavailable,
                format!("{origin} has no versions of {org} after {current}"),
            ));
        }
        for version in fetched.versions {
            receive(node, version).await?;
        }
        // An answer that moves nothing on (versions this node already has) would ask again forever.
        if load(node, org)?.body.version <= current {
            return Err(Error::new(
                ErrorKind::Unavailable,
                format!("{origin} sent no version of {org} after {current}"),
            ));
        }
    }
}
