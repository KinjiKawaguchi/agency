//! Root changes held for the owner (docs/protocol.md, "Pending root changes"). When the root key needs the
//! owner's presence, a root's node never signs because someone else asked (a join, a device change): it
//! checks the request as before, holds it, and answers `202` with a handle. The owner sees every held change
//! and signs them in one run (`agency root-sign`), after one presence prompt that names them. A joiner keeps
//! its handle and fetches the signed version with it; a member's node takes it by catch-up.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::unseen;
use agency_proto::clock::{self, Duration};
use agency_proto::device::{DeviceChange, DeviceOp};
use agency_proto::invite::{code_from_bytes, JoinRequest};
use agency_proto::org::SignedOrg;
use agency_proto::root_change::{
    waiting_message, PendingRootAnswer, RootAnswer, RootChangeKind, RootChangeRef,
    PENDING_ROOT_DAYS,
};
use agency_proto::{Id, Signed};
use serde::{Deserialize, Serialize};

/// A change someone asked the root to sign, checked and held.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRootChange {
    /// 32 random bytes as base64url; the only handle to it.
    pub id: String,
    pub org: Id,
    pub received_at: String,
    /// The order it came in among the held changes, so a run signs them in that order even within a second.
    #[serde(default)]
    pub order: u64,
    pub change: RootChange,
    /// The version that carried it, once signed; kept until it expires so a joiner can fetch it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_version: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RootChange {
    Join { request: Box<Signed<JoinRequest>> },
    Device { change: Box<Signed<DeviceChange>> },
}

/// A join this node asked for, held by the org's root node: where to ask, with which handle, and the
/// definition it checked the invite against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitingJoin {
    pub org: Id,
    pub node: String,
    pub pending: String,
    pub current: SignedOrg,
}

impl PendingRootChange {
    /// What it changes, in words, for the terminal, the desk, and the presence prompt. A label came from
    /// someone else: a control or format character in it is shown as `?`, so it can neither move the terminal's cursor
    /// nor push a name out of the prompt.
    pub fn what(&self) -> String {
        printable(&self.raw_what())
    }

    fn raw_what(&self) -> String {
        match &self.change {
            RootChange::Join { request } => {
                format!(
                    "add member {} ({})",
                    request.body.account, request.body.label
                )
            }
            RootChange::Device { change } => match &change.body.op {
                DeviceOp::Add { device } => {
                    format!("add device {} for {}", device.label, change.body.account)
                }
                DeviceOp::Remove { device } => {
                    format!("remove device {device} of {}", change.body.account)
                }
            },
        }
    }

    /// What a notification names the change by: 16 hex digits of SHA-256("agency/push-id/v1" || handle). The
    /// handle itself is how a joiner fetches its version, so it never goes to a push service, and the derivation
    /// is its own, so the id shares nothing with the key the store keeps the change under.
    pub fn notice_id(&self) -> String {
        let text = format!("agency/push-id/v1{}", self.id);
        agency_proto::digest::sha256_hex(text.as_bytes())[..16].to_owned()
    }

    /// The account the change is for: the joiner, or the member whose device it is.
    pub fn account(&self) -> &Id {
        match &self.change {
            RootChange::Join { request } => &request.body.account,
            RootChange::Device { change } => &change.body.account,
        }
    }

    fn is_expired(&self, node: &Node) -> bool {
        let end = clock::parse(&self.received_at).map(|at| at + Duration::days(PENDING_ROOT_DAYS));
        end.is_none_or(|end| end <= node.now())
    }

    pub fn as_ref_for_desk(&self) -> RootChangeRef {
        let (kind, label, device) = match &self.change {
            RootChange::Join { request } => (
                RootChangeKind::AddMember,
                Some(request.body.label.clone()),
                None,
            ),
            RootChange::Device { change } => match &change.body.op {
                DeviceOp::Add { device } => (
                    RootChangeKind::AddDevice,
                    Some(device.label.clone()),
                    Some(device.id.clone()),
                ),
                DeviceOp::Remove { device } => {
                    (RootChangeKind::RemoveDevice, None, Some(device.clone()))
                }
            },
        };
        RootChangeRef {
            id: self.id.clone(),
            org: self.org.clone(),
            kind,
            member: self.account().clone(),
            label,
            device,
            what: self.what(),
            received_at: self.received_at.clone(),
        }
    }
}

use agency_proto::org::printable;

/// The most changes one presence prompt signs: every one of them is named in its words.
pub const MOST_PER_PROMPT: usize = 3;

/// The most changes held at once for one account in one org: more wait for these to be signed or to expire.
pub const MOST_HELD_PER_ACCOUNT: usize = 5;

/// Whether the root's node must hold a change for the owner instead of signing it now: its root key asks
/// for presence. A root that is still the account key signs without asking.
pub(crate) fn must_hold(node: &Node, current: &SignedOrg) -> bool {
    let root = &current.body.root.public_key;
    node.root_keys().needs_presence() && root != &node.key().public()
}

/// Holds a checked change and answers with its handle.
pub(crate) fn hold(node: &Node, org: &Id, change: RootChange) -> Result<RootAnswer> {
    let account = match &change {
        RootChange::Join { request } => request.body.account.clone(),
        RootChange::Device { change } => change.body.account.clone(),
    };
    let held_for = waiting(node)?
        .iter()
        .filter(|one| &one.org == org && one.account() == &account)
        .count();
    if held_for >= MOST_HELD_PER_ACCOUNT {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!(
                "{MOST_HELD_PER_ACCOUNT} changes for {account} already wait for the owner of {org}"
            ),
        ));
    }
    let id = format!(
        "{}{}",
        code_from_bytes(node.random()),
        code_from_bytes(node.random())
    );
    let last = node
        .store()
        .pending_roots()
        .iter()
        .map(|one| one.order)
        .max();
    node.store().put_pending_root(&PendingRootChange {
        id: id.clone(),
        org: org.clone(),
        received_at: node.now_text(),
        order: last.map_or(1, |last| last + 1),
        change,
        signed_version: None,
    })?;
    Ok(RootAnswer::Pending(PendingRootAnswer {
        pending: id,
        message: waiting_message(),
    }))
}

/// The changes that wait for the owner, oldest first; expired ones are dropped on the way.
pub fn waiting(node: &Node) -> Result<Vec<PendingRootChange>> {
    let mut all = node.store().pending_roots();
    all.sort_by_key(|one| one.order);
    let mut kept = vec![];
    for one in all {
        if one.is_expired(node) {
            crate::invites::release_held_invite(node, &one)?;
            node.store().remove_pending_root(&one.id)?;
        } else if one.signed_version.is_none() {
            kept.push(one);
        }
    }
    Ok(kept)
}

/// Drops a held change unsigned; a join's invite can be used again.
pub fn refuse(node: &Node, id: &str) -> Result<()> {
    let held = node
        .store()
        .pending_roots()
        .into_iter()
        .find(|one| one.id == id && one.signed_version.is_none())
        .ok_or_else(|| Error::not_found(format!("no held change {id}")))?;
    crate::invites::release_held_invite(node, &held)?;
    node.store().remove_pending_root(id)
}

/// The answer to a joiner asking for its held join by handle: still pending, or the version that signed it.
/// Anything else, an unknown handle included, is not found.
pub fn join_answer(node: &Node, org: &Id, id: &str) -> Result<RootAnswer> {
    let held = node
        .store()
        .pending_roots()
        .into_iter()
        .find(|one| &one.org == org && one.id == id && !one.is_expired(node))
        .filter(|one| matches!(one.change, RootChange::Join { .. }));
    let Some(held) = held else {
        return Err(unseen::refused(format!(
            "no held join of {org} with that handle"
        )));
    };
    match held.signed_version {
        None => Ok(RootAnswer::Pending(PendingRootAnswer {
            pending: held.id,
            message: waiting_message(),
        })),
        Some(version) => node
            .store()
            .load_org_version(org, version)
            .map(|doc| RootAnswer::Signed(Box::new(doc)))
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "not found")),
    }
}

/// Marks a held change signed by `version`, or drops it when it could not be signed any more.
pub(crate) fn settle(node: &Node, held: &PendingRootChange, version: Option<u64>) -> Result<()> {
    match version {
        Some(version) => node.store().put_pending_root(&PendingRootChange {
            signed_version: Some(version),
            ..held.clone()
        }),
        None => {
            crate::invites::release_held_invite(node, held)?;
            node.store().remove_pending_root(&held.id)
        }
    }
}

/// What one `agency root-sign` run did in one org.
#[derive(Debug, Default)]
pub struct SignRun {
    /// Each held change and the version that carried it.
    pub signed: Vec<(PendingRootChange, SignedOrg)>,
    /// Each held change that could not be signed any more, and why (the member left, the device was
    /// revoked, the joiner is in by now).
    pub dropped: Vec<(PendingRootChange, String)>,
}

/// The words of the presence prompt for `held`: how many, the org, and every one of them (at most
/// `MOST_PER_PROMPT`), so that one press never approves something the prompt did not name.
pub fn purpose_of(org: &Id, held: &[PendingRootChange]) -> String {
    let named: Vec<String> = held.iter().map(PendingRootChange::what).collect();
    format!(
        "Sign {} change{} for {org}: {}",
        held.len(),
        if held.len() == 1 { "" } else { "s" },
        named.join(", ")
    )
}

/// Signs, in one run after one presence prompt, the held changes of `org` whose handles are `ids` (the ones
/// the owner was shown), each as its own version, in the order they came.
pub async fn sign_held(node: &Node, org: &Id, ids: &[String]) -> Result<SignRun> {
    if ids.len() > MOST_PER_PROMPT {
        return Err(Error::bad_request(format!(
            "one prompt signs at most {MOST_PER_PROMPT} changes, each named"
        )));
    }
    let _guard = node.write_lock().lock().await;
    let held: Vec<PendingRootChange> = waiting(node)?
        .into_iter()
        .filter(|one| &one.org == org && ids.contains(&one.id))
        .collect();
    let current = crate::orgs::load(node, org)?;
    if !crate::orgs::is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("this node does not hold the root key of org {org}"),
        ));
    }
    if held.is_empty() {
        return Ok(SignRun::default());
    }
    let run = run_signatures(node, &current, &held)?;
    for (one, doc) in &run.signed {
        crate::orgs::publish(node, doc)?;
        settle(node, one, Some(doc.body.version))?;
    }
    for (one, _) in &run.dropped {
        settle(node, one, None)?;
    }
    Ok(run)
}

fn run_signatures(node: &Node, current: &SignedOrg, held: &[PendingRootChange]) -> Result<SignRun> {
    let org = &current.body.org;
    let key = current.body.root.public_key.clone();
    let mut run = SignRun::default();
    let mut work = |sign: &crate::ports::RootSigner| -> std::result::Result<(), String> {
        let mut doc = current.clone();
        for one in held {
            let next =
                next_body(node, &doc, one).and_then(|body| crate::orgs::sign_by(node, body, sign));
            match next {
                Ok(signed) => {
                    doc = signed.clone();
                    run.signed.push((one.clone(), signed));
                }
                Err(error) if error.kind == ErrorKind::Forbidden => return Err(error.message),
                Err(error) => run.dropped.push((one.clone(), error.message)),
            }
        }
        Ok(())
    };
    let answer = if key == node.key().public() {
        let account = |text: &[u8]| Ok(node.key().sign(text));
        work(&account)
    } else {
        node.root_keys()
            .with_signer(org, &key, &purpose_of(org, held), &mut work)
    };
    answer.map_err(|why| {
        Error::new(
            ErrorKind::Forbidden,
            format!("the root key of {org} did not sign: {why}"),
        )
    })?;
    Ok(run)
}

/// The version a held change makes after `doc`, checked again against it: as it was when it came, for its
/// time, and against what the org is now, for everything else.
fn next_body(
    node: &Node,
    doc: &SignedOrg,
    held: &PendingRootChange,
) -> Result<agency_proto::OrgDefinition> {
    match &held.change {
        RootChange::Join { request } => {
            let record = crate::invites::held_invite(node, &request.body)
                .ok_or_else(|| Error::bad_request("its invite is no longer held for it"))?;
            crate::invites::joined_body(node, doc, &request.body, &record)
        }
        RootChange::Device { change } => {
            let received = clock::parse(&held.received_at)
                .ok_or_else(|| Error::bad_request("a held change without a time"))?;
            crate::devices::changed_body(node, doc, change, received)
        }
    }
}
