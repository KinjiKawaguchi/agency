//! An org's change history in words (docs/protocol.md, "Org definition"): every version this node keeps, which key
//! signed it, and what it changed. The versions are signed and each names the one before it, so this is a record
//! no one can rewrite; these are only its words.
use crate::change_words::every_change;
use crate::error::Result;
use crate::node::Node;
use crate::orgs::load;
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::org::SignedOrg;
use agency_proto::{Id, Kind};
use serde::Serialize;

/// Whether a version's signature was checked, and what it showed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SignatureCheck {
    /// Made by the root key or the recovery key of the version just before it (of the version itself, for the first).
    Valid,
    /// Made by no key the version just before it names: the version was changed after it was signed, or never signed
    /// by the org's root.
    Invalid,
    /// Not checked: the version just before it is not kept here, so which keys could sign it is not known (the missing
    /// versions may have moved the root).
    NotChecked,
}

/// One version of the org as the history shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub version: u64,
    /// When the root signed it (`at`); absent for a version this node does not keep.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// What its signature showed; absent for a version this node does not keep.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureCheck>,
    /// Which key signed it, by fingerprint (`root key …` or `recovery key …`), when the signature is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
    /// Whether its `prev` is the digest of the version just before it (a version with another `prev` is a fork of the
    /// org's history); absent when that version is not kept here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follows: Option<bool>,
    /// The version its changes are counted from, when that is not the one just before it: after versions this node does
    /// not keep, the changes are those of all of them together.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compared_with: Option<u64>,
    /// What it changed, in words, from the version it is compared with.
    pub changes: Vec<String>,
    /// Whether this node keeps the version. One it does not keep is listed, so that a gap is never silent.
    pub is_kept: bool,
}

/// Which key of `before` (or, for the first version, of the version itself) signed `doc`.
fn signer_of(before: Option<&SignedOrg>, doc: &SignedOrg) -> Option<String> {
    let root = &before.unwrap_or(doc).body.root;
    if doc.verify(Kind::Org, &root.public_key) {
        return Some(format!("root key {}", root.public_key.fingerprint()));
    }
    root.recovery
        .as_ref()
        .filter(|key| doc.verify(Kind::Org, key))
        .map(|key| format!("recovery key {}", key.fingerprint()))
}

/// The entry of `doc`, given the last version before it that this node keeps (`kept_before`).
fn entry(kept_before: Option<&SignedOrg>, doc: &SignedOrg) -> HistoryEntry {
    let version = doc.body.version;
    // The version just before: none for the first, and unknown when the last kept one is older.
    let just_before = kept_before.filter(|before| before.body.version + 1 == version);
    let is_known = version == 1 || just_before.is_some();
    let signed_by = is_known.then(|| signer_of(just_before, doc)).flatten();
    let signature = match (is_known, &signed_by) {
        (false, _) => SignatureCheck::NotChecked,
        (true, Some(_)) => SignatureCheck::Valid,
        (true, None) => SignatureCheck::Invalid,
    };
    let follows = match just_before {
        Some(before) => Some(doc.body.prev.as_deref() == Some(before.digest().as_str())),
        None if version == 1 => Some(doc.body.prev.is_none()),
        None => None,
    };
    let changes = match kept_before {
        Some(before) => every_change(&before.body, &doc.body),
        None => vec![format!(
            "create the org {}",
            agency_proto::org::printable(&doc.body.label)
        )],
    };
    HistoryEntry {
        version,
        at: Some(doc.body.at.clone()),
        signature: Some(signature),
        signed_by,
        follows,
        compared_with: kept_before
            .filter(|_| just_before.is_none())
            .map(|before| before.body.version),
        changes,
        is_kept: true,
    }
}

/// The history of `org` from its first version to the one this node holds now, oldest first; with `since`, only the
/// versions signed at or after it.
pub fn org_history(
    node: &Node,
    org: &Id,
    since: Option<OffsetDateTime>,
) -> Result<Vec<HistoryEntry>> {
    let current = load(node, org)?;
    let mut before: Option<SignedOrg> = None;
    let mut history = vec![];
    for version in 1..=current.body.version {
        let Some(doc) = node.store().load_org_version(org, version) else {
            history.push(HistoryEntry {
                version,
                at: None,
                signature: None,
                signed_by: None,
                follows: None,
                compared_with: None,
                changes: vec![],
                is_kept: false,
            });
            continue;
        };
        history.push(entry(before.as_ref(), &doc));
        before = Some(doc);
    }
    let is_since = |entry: &HistoryEntry| {
        let at = entry.at.as_deref().and_then(clock::parse);
        since.is_none_or(|since| at.is_none_or(|at| at >= since))
    };
    Ok(history.into_iter().filter(is_since).collect())
}
