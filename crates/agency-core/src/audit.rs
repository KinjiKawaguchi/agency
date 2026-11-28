//! An org's change history in words (docs/protocol.md, "Org definition"): every version this node keeps, which key
//! signed it, and what it changed. The versions are signed and each names the one before it, so this is a record
//! no one can rewrite; these are only its words.
use crate::change_words::every_change;
use crate::error::Result;
use crate::node::Node;
use crate::orgs::load;
use agency_proto::clock::{self, OffsetDateTime};
pub use agency_proto::history::{HistoryEntry, SignatureCheck};
use agency_proto::org::SignedOrg;
use agency_proto::{Id, Kind};

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

/// What a read of the history asks for.
#[derive(Clone, Copy, Debug)]
pub struct HistoryQuery {
    /// Only the versions signed at or after this time (a version this node does not keep is always listed).
    pub since: Option<OffsetDateTime>,
    /// Only the versions above this number.
    pub after: u64,
    /// At most this many entries.
    pub limit: usize,
}

impl HistoryQuery {
    /// Every version, as `agency audit` lists them.
    pub fn all(since: Option<OffsetDateTime>) -> Self {
        Self {
            since,
            after: 0,
            limit: usize::MAX,
        }
    }
}

/// The entries a read asked for, and whether more would have followed them.
pub struct HistoryPage {
    pub entries: Vec<HistoryEntry>,
    pub is_cut: bool,
}

/// The history of `org` from its first version to the one this node holds now, oldest first, as `query` asks. A
/// version that is not returned (at or below `after`, or signed before `since`) is loaded, since the next one is
/// compared with it, but its signature is not checked and its changes are not worded.
pub fn org_history_page(node: &Node, org: &Id, query: &HistoryQuery) -> Result<HistoryPage> {
    let current = load(node, org)?;
    let mut before: Option<SignedOrg> = None;
    let mut entries = vec![];
    for version in 1..=current.body.version {
        let doc = node.store().load_org_version(org, version);
        let is_wanted = version > query.after
            && doc.as_ref().is_none_or(|doc| {
                let at = clock::parse(&doc.body.at);
                query
                    .since
                    .is_none_or(|since| at.is_none_or(|at| at >= since))
            });
        if is_wanted && entries.len() >= query.limit {
            return Ok(HistoryPage {
                entries,
                is_cut: true,
            });
        }
        match doc {
            None if is_wanted => entries.push(HistoryEntry {
                version,
                at: None,
                signature: None,
                signed_by: None,
                follows: None,
                compared_with: None,
                changes: vec![],
                is_kept: false,
            }),
            None => {}
            Some(doc) => {
                if is_wanted {
                    entries.push(entry(before.as_ref(), &doc));
                }
                before = Some(doc);
            }
        }
    }
    Ok(HistoryPage {
        entries,
        is_cut: false,
    })
}

/// The history of `org` from its first version to the one this node holds now, oldest first; with `since`, only the
/// versions signed at or after it.
pub fn org_history(
    node: &Node,
    org: &Id,
    since: Option<OffsetDateTime>,
) -> Result<Vec<HistoryEntry>> {
    org_history_page(node, org, &HistoryQuery::all(since)).map(|page| page.entries)
}
