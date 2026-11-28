//! What an export holds, read from this node's stores. Nothing here reads a key: the account key, the CLI
//! device's key and the org's root key are not in any of these stores' records, and the org definition holds
//! public keys only.
use super::manifest::ExportFile;
use agency_core::Node;
use agency_proto::org::SignedOrg;
use agency_proto::Id;
use anyhow::{bail, Result};

/// Where each kept version of the org is, in the export's directory.
pub fn version_path(version: u64) -> String {
    format!("versions/{version:06}.json")
}

pub const DECISIONS_PATH: &str = "decisions.json";
pub const OUTCOMES_PATH: &str = "outcomes.json";

/// Every version of the org this node kept, oldest first. A node that joined later keeps them from the version
/// it joined at, and the last is the definition it holds now.
pub fn kept_versions(node: &Node, org: &Id) -> Result<Vec<SignedOrg>> {
    let store = node.store();
    let Some(current) = store.load_org(org) else {
        bail!("this node holds no org {org}");
    };
    let chain: Vec<SignedOrg> = (1..=current.body.version)
        .filter_map(|version| store.load_org_version(org, version))
        .collect();
    let is_unbroken = chain
        .windows(2)
        .all(|pair| pair[1].body.version == pair[0].body.version + 1);
    if !is_unbroken {
        bail!("this node's history of org {org} has a version missing");
    }
    match chain.last() {
        Some(last) if last.digest() == current.digest() => Ok(chain),
        _ => bail!("the definition of org {org} is not the last version this node kept"),
    }
}

/// One file for each version, as it was signed.
pub fn version_files(chain: &[SignedOrg]) -> Result<Vec<ExportFile>> {
    chain
        .iter()
        .map(|doc| ExportFile::json(version_path(doc.body.version), doc))
        .collect()
}

/// The decisions this node kept for the org, oldest first.
pub fn decisions(node: &Node, org: &Id) -> Result<ExportFile> {
    let mut records = node.store().recent_decisions(usize::MAX);
    records.retain(|record| &record.org == org);
    records.reverse();
    ExportFile::json(DECISIONS_PATH, &records)
}

/// The owner's numbers for the org, in the order they were entered, when this node keeps any: they are kept
/// on the owner's node only, so no other node has a file for them.
pub fn outcomes(node: &Node, org: &Id) -> Result<Option<ExportFile>> {
    let observations = node.store().observations(org);
    if observations.is_empty() {
        return Ok(None);
    }
    Ok(Some(ExportFile::json(OUTCOMES_PATH, &observations)?))
}
