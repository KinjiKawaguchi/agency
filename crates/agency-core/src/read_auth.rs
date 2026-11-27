//! Signed reads (kind `read`): the reader signs the method, path, time, and account, and, for a node that can
//! take it, the id of the node it reads (the text of `agency/v2/read`). The reading node checks the signature
//! against the org definition and answers only members of the org.
use crate::error::Result;
use crate::node::Node;
use crate::routes;
use crate::unseen;
use agency_proto::about::Capability;
use agency_proto::client::node_id;
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::org::OrgDefinition;
use agency_proto::signed::{is_node_id, read_text, read_text_for};
use agency_proto::{AccountKey, Id, Kind, Person, PublicKey};

/// How far a signed time may be from the reading node's clock, either way. It also limits how long
/// a captured signature can be reused.
pub const SKEW_SECONDS: i64 = 300;

/// The read signature as it travels: account, time, signature, and the node it names, unchecked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadAuth {
    pub account: String,
    pub date: String,
    pub signature: String,
    /// The id of the node the read is made for (`x-agency-audience`), when the signature covers it.
    pub audience: Option<String>,
}

/// A read that names no node (`agency/v1/read`).
pub fn sign_read(
    key: &AccountKey,
    account: &Id,
    method: &str,
    path: &str,
    now: OffsetDateTime,
) -> ReadAuth {
    let date = clock::format_millis(now);
    let signature = key.sign(&Kind::Read.message(&read_text(method, path, &date, account)));
    ReadAuth {
        account: account.to_string(),
        date,
        signature,
        audience: None,
    }
}

/// A read made for the node whose id is `audience` (`agency/v2/read`): no other node takes it.
pub fn sign_read_for(
    key: &AccountKey,
    account: &Id,
    method: &str,
    path: &str,
    now: OffsetDateTime,
    audience: &str,
) -> ReadAuth {
    let date = clock::format_millis(now);
    let text = read_text_for(method, path, &date, account, audience);
    ReadAuth {
        account: account.to_string(),
        date,
        signature: key.sign(&Kind::ReadFor.message(&text)),
        audience: Some(audience.to_owned()),
    }
}

/// The read of `path` that this node makes of the peer at `origin`, a node of `org`: made for that node when it
/// lists `readAudience` (from the `about` this node kept of it), and not naming it otherwise. The read of the
/// peer's `about` never names it: it is how the capability is learned.
pub fn sign_for_peer(node: &Node, org: &Id, origin: &str, path: &str) -> ReadAuth {
    let key = node.key();
    let account = node.account();
    let now = node.now();
    match audience_of_peer(node, org, origin, path) {
        Some(audience) => sign_read_for(key, &account, "GET", path, now, &audience),
        None => sign_read(key, &account, "GET", path, now),
    }
}

fn audience_of_peer(node: &Node, org: &Id, origin: &str, path: &str) -> Option<String> {
    if path == routes::about(org) {
        return None;
    }
    let kept = node.store().peer_abouts(org);
    let about = kept.get(origin)?.about.as_ref()?;
    if !about.lists(Capability::ReadAudience) {
        return None;
    }
    node_id(origin)
}

/// Why a node takes no read that names no node from an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnboundRefusal {
    /// The node is set to require reads that name it (`federation.requireReadAudience`).
    Required,
    /// The node has taken a read that names it from this account, so it takes no other kind from it.
    Ratcheted,
}

/// What a node takes besides reads that name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadPolicy<'a> {
    /// This node's id (`node_id` of its `url`); none when `url` is no node origin, and the node then takes
    /// no read that names a node.
    pub own_id: Option<&'a str>,
    /// Whether a read that names no node is taken, or why it is not.
    pub unbound: Result<(), UnboundRefusal>,
}

impl ReadPolicy<'_> {
    /// A node that takes both kinds and names itself `own_id`.
    pub fn open(own_id: Option<&str>) -> ReadPolicy<'_> {
        ReadPolicy {
            own_id,
            unbound: Ok(()),
        }
    }
}

/// Checks a signed read and returns the member who signed it. `definition` is the org's, if this node
/// has it; `is_here` says whether what is read is on this node (the team runs here, or, for the
/// definition itself, the org is here). Every failure is the same `NotFound` (see `unseen`): a
/// malformed or stale signature, no definition, a non-member, a wrong signature, or nothing here. Once
/// the account and time are well formed, one signature check is spent whatever is missing.
///
/// A read that names a node (`auth.audience`) is checked over the text of `agency/v2/read` and taken only by the
/// node it names (`policy.own_id`). One that names none is checked over `agency/v1/read` and taken as
/// `policy.unbound` says. The reason of such a refusal is in the log, never in the answer.
pub fn verify_read<'a>(
    definition: Option<&'a OrgDefinition>,
    is_here: bool,
    auth: &ReadAuth,
    method: &str,
    path: &str,
    now: OffsetDateTime,
    policy: &ReadPolicy,
) -> Result<&'a Person> {
    let account = Id::parse(&auth.account)
        .map_err(|_| unseen::refused("a signed read without a well-formed account"))?;
    let at = clock::parse(&auth.date).ok_or_else(|| {
        unseen::refused(format!(
            "a signed read by {account} without a well-formed time"
        ))
    })?;
    let person = definition.and_then(|definition| definition.member(&account));
    let key: &PublicKey = match person {
        // A hosted member has no account key and never signs a read: the stand-in key fails it, as it does a
        // stranger's.
        Some(person) => person
            .public_key
            .as_ref()
            .unwrap_or_else(|| unseen::stand_in_key()),
        None => unseen::stand_in_key(),
    };
    let message = match &auth.audience {
        Some(audience) => {
            Kind::ReadFor.message(&read_text_for(method, path, &auth.date, &account, audience))
        }
        None => Kind::Read.message(&read_text(method, path, &auth.date, &account)),
    };
    let is_valid = key.verify(&message, &auth.signature);
    let names_another_node = auth
        .audience
        .as_deref()
        .is_some_and(|audience| policy.own_id != Some(audience));
    let skew = (now - at).whole_seconds();
    match (definition, person) {
        (None, _) => Err(unseen::refused(format!(
            "a read by {account} in an org this node does not have"
        ))),
        (Some(definition), None) => Err(unseen::refused(format!(
            "a read by {account}, who is not a member of {}",
            definition.org
        ))),
        (Some(_), Some(_)) if !is_valid => Err(unseen::refused(format!(
            "a read by {account} whose signature does not match their key"
        ))),
        (Some(_), Some(_)) if skew.abs() > SKEW_SECONDS => Err(unseen::refused_member(format!(
            "a read signed by {account} at {}, {} seconds {} this node's clock; check the clocks",
            clock::format(at),
            skew.abs(),
            if skew > 0 { "behind" } else { "ahead of" }
        ))),
        (Some(_), Some(_)) if names_another_node => Err(unseen::refused_member(format!(
            "a read signed by {account} for a node that is not this one ({}); this node is {}",
            auth.audience
                .as_deref()
                .filter(|audience| is_node_id(audience))
                .unwrap_or("not written as a node id"),
            policy
                .own_id
                .unwrap_or("not named: its url is no node origin")
        ))),
        (Some(_), Some(_)) if auth.audience.is_none() && policy.unbound.is_err() => {
            Err(unseen::refused_member(refused_unbound(&account, policy)))
        }
        (Some(definition), Some(_)) if !is_here => Err(unseen::refused_member(format!(
            "a read by {account} of something in {} that is not on this node",
            definition.org
        ))),
        (Some(_), Some(person)) => Ok(person),
    }
}

/// Why a read that names no node is refused, and how to take it again, in words for the owner's log.
fn refused_unbound(account: &Id, policy: &ReadPolicy) -> String {
    let why = match policy.unbound {
        Err(UnboundRefusal::Required) => {
            "this node is set to take only reads that name it (federation.requireReadAudience)".to_owned()
        }
        _ => format!(
            "this node has taken a read from {account} that names it, so it takes no other kind from {account} any more"
        ),
    };
    format!(
        "a read signed by {account} that names no node; {why}. If {account} also reads from a node that is not updated yet, update that node, or list {account} in federation.unboundReadAccounts in config.json to take its older reads again"
    )
}

/// `verify_read`, accepted once per signature (`Node::accept_read_once`): a copy of a valid read sent
/// again is refused like any failed read. A read that names this node is remembered as the account's
/// (`Node::keep_bound_reader`), except the read of `about`.
pub fn verify_read_once<'a>(
    node: &Node,
    definition: Option<&'a OrgDefinition>,
    is_here: bool,
    auth: &ReadAuth,
    method: &str,
    path: &str,
) -> Result<&'a Person> {
    let config = node.config();
    let own_id = node_id(&config.url);
    let is_about = definition.is_some_and(|definition| path == routes::about(&definition.org));
    let account = Id::parse(&auth.account).ok();
    let unbound = match &account {
        Some(account) if !is_about => unbound_policy(node, &config, account),
        _ => Ok(()),
    };
    let policy = ReadPolicy {
        own_id: own_id.as_deref(),
        unbound,
    };
    let person = verify_read(definition, is_here, auth, method, path, node.now(), &policy)?;
    let signed_at = clock::parse(&auth.date)
        .ok_or_else(|| unseen::refused("a signed read without a well-formed time"))?;
    if !node.accept_read_once(&auth.signature, signed_at) {
        // Anyone who saw the read can send the copy, so it is not vouched for by the member.
        return Err(unseen::refused(format!(
            "a copy of a read by {} that was already accepted",
            person.id
        )));
    }
    if let (Some(account), true) = (&account, auth.audience.is_some() && !is_about) {
        node.change_bound_readers(|kept| {
            kept.insert(account.clone());
        });
    }
    Ok(person)
}

fn unbound_policy(
    node: &Node,
    config: &crate::config::NodeConfig,
    account: &Id,
) -> Result<(), UnboundRefusal> {
    if config.federation.unbound_read_accounts.contains(account) {
        return Ok(());
    }
    if config.federation.require_read_audience {
        return Err(UnboundRefusal::Required);
    }
    if node
        .bound_readers()
        .is_ok_and(|kept| kept.contains(account))
    {
        return Err(UnboundRefusal::Ratcheted);
    }
    Ok(())
}

/// Forgets the accounts that are no longer members of any org this node has: the ratchet is kept only for the
/// members of the orgs the node holds, so it stays as small as they are, and a member who leaves and comes back
/// starts without it. Called whenever a definition is kept (`push_devices::keep_org`).
pub(crate) fn drop_departed_readers(node: &Node) {
    let members: std::collections::BTreeSet<Id> = node
        .store()
        .orgs()
        .iter()
        .flat_map(|doc| doc.body.people.iter().map(|one| one.account.clone()))
        .collect();
    node.change_bound_readers(|kept| kept.retain(|account| members.contains(account)));
}

/// What `agency status` says of how this node takes signed reads, one line each, and none when it takes them all
/// in the usual way: that its `url` gives it no id, that it requires reads that name it, the accounts it takes
/// no older read from, and the accounts it still takes older reads from (an exception that is easy to forget).
pub fn status_lines(node: &Node) -> Vec<String> {
    let config = node.config();
    let listed = &config.federation.unbound_read_accounts;
    let names = |accounts: Vec<&Id>| {
        accounts
            .iter()
            .map(|account| account.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut lines = vec![];
    if node_id(&config.url).is_none() {
        lines.push(format!(
            "Signed reads: {} is not written as https://<host>[:<port>] (or http:// on a loopback address), so other nodes cannot name this node in a read, and it does not list readAudience",
            config.url
        ));
    }
    if config.federation.require_read_audience {
        lines.push("Signed reads: only reads that name this node are taken (federation.requireReadAudience); the read of about is the exception".to_owned());
    }
    match node.store().bound_readers() {
        Err(error) => lines.push(format!(
            "Signed reads: bound-readers.json cannot be read ({error}); it is left as it is, and reads that name no node are taken as before until it is mended or removed"
        )),
        Ok(kept) => {
            let refused: Vec<Id> = kept
                .into_iter()
                .filter(|account| !listed.contains(account))
                .collect();
            if !refused.is_empty() && !config.federation.require_read_audience {
                lines.push(format!(
                    "Signed reads: no read that names no node is taken from {} (each has named this node)",
                    names(refused.iter().collect())
                ));
            }
        }
    }
    if !listed.is_empty() {
        lines.push(format!(
            "Signed reads: older reads are still taken from {} (federation.unboundReadAccounts); take the account out of the list once its nodes are updated",
            names(listed.iter().collect())
        ));
    }
    lines
}
