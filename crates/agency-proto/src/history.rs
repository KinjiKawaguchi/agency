//! An org's change history in words (docs/protocol.md, "Org definition"): every version a node keeps, which key
//! signed it, and what it changed. `agency audit --json` prints one `HistoryEntry` per line and
//! `GET /api/orgs/<org>/history` answers them as `OrgHistory`; both are worked out by the same function of the
//! node, so the two never disagree.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether a version's signature was checked, and what it showed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct HistoryEntry {
    #[ts(type = "number")]
    pub version: u64,
    /// When the root signed it (`at`); absent for a version this node does not keep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// What its signature showed; absent for a version this node does not keep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureCheck>,
    /// Which key signed it, by fingerprint (`root key ...` or `recovery key ...`), when the signature is valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
    /// Whether its `prev` is the digest of the version just before it (a version with another `prev` is a fork of the
    /// org's history); absent when that version is not kept here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<bool>,
    /// The version its changes are counted from, when that is not the one just before it: after versions this node does
    /// not keep, the changes are those of all of them together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub compared_with: Option<u64>,
    /// What it changed, in words, from the version it is compared with.
    pub changes: Vec<String>,
    /// Whether this node keeps the version. One it does not keep is listed, so that a gap is never silent.
    pub is_kept: bool,
}

/// The entries a read of the history returns when it names no `limit`.
pub const DEFAULT_LIMIT: usize = 200;
/// The most entries a read of the history may ask for.
pub const MOST_LIMIT: usize = 1000;

/// `GET /api/orgs/<org>/history`: the entries `agency audit --json` prints, oldest first, at most `limit` of them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct OrgHistory {
    pub entries: Vec<HistoryEntry>,
    /// Entries after the last one were left out because of `limit`: ask again with `after` set to its `version`.
    /// Left out when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, type = "boolean")]
    pub more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SAMPLE: &str = include_str!("../../../testdata/history.sample.json");

    #[test]
    fn the_sample_survives_a_round_trip_and_leaves_out_what_has_no_value() {
        let history: OrgHistory = serde_json::from_str(SAMPLE).unwrap();
        let again = serde_json::to_value(&history).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        assert!(
            !SAMPLE.contains("null"),
            "an optional field is left out, never null"
        );
    }

    #[test]
    fn the_sample_has_a_kept_version_a_gap_and_a_version_compared_across_it() {
        let history: OrgHistory = serde_json::from_str(SAMPLE).unwrap();
        let entries = &history.entries;
        assert_eq!(entries[0].signature, Some(SignatureCheck::Valid));
        assert!(!entries[1].is_kept && entries[1].at.is_none());
        assert_eq!(entries[2].signature, Some(SignatureCheck::NotChecked));
        assert_eq!(entries[2].compared_with, Some(1));
    }

    #[test]
    fn more_is_written_only_when_there_is_more() {
        let history: OrgHistory = serde_json::from_str(SAMPLE).unwrap();
        assert!(history.more);
        let none = OrgHistory {
            more: false,
            ..history
        };
        assert!(serde_json::to_value(&none).unwrap().get("more").is_none());
    }

    #[test]
    fn a_signature_check_is_written_in_camel_case() {
        assert_eq!(
            serde_json::to_value(SignatureCheck::NotChecked).unwrap(),
            "notChecked"
        );
    }
}
