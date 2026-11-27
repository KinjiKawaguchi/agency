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

/// One version of the org as the history shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub version: u64,
    /// When the root signed it (`at`); absent for a version this node does not keep.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Which key signed it, by fingerprint: `root key …` or `recovery key …`. Absent for a version this node does not
    /// keep, or one no key of the version before signed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
    /// What it changed, in words, from the version before it that this node keeps.
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

fn entry(before: Option<&SignedOrg>, doc: &SignedOrg) -> HistoryEntry {
    let changes = match before {
        Some(before) => every_change(&before.body, &doc.body),
        None => vec![format!(
            "create the org {}",
            agency_proto::org::printable(&doc.body.label)
        )],
    };
    HistoryEntry {
        version: doc.body.version,
        at: Some(doc.body.at.clone()),
        signed_by: signer_of(before, doc),
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
                signed_by: None,
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
