//! Checking an export on its own: the manifest against the files, and the org's versions as a chain of
//! signatures. It needs no node and no key. What it proves is that the files are the ones the manifest lists and
//! that every version of the org is signed as the protocol says; the manifest itself is not signed, and the
//! decisions and numbers are this node's own records, which no one else signed.
use super::collect::{version_path, DECISIONS_PATH, OUTCOMES_PATH};
use super::manifest::{Manifest, FORMAT, MANIFEST_PATH};
use agency_core::decisions::DecisionRecord;
use agency_proto::digest::sha256_hex;
use agency_proto::observation::Observation;
use agency_proto::org::{verify_genesis, verify_successor, SignedOrg};
use agency_proto::Kind;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// What a verified export holds.
#[derive(Debug, PartialEq, Eq)]
pub struct Verified {
    pub manifest: Manifest,
    pub decisions: usize,
    pub numbers: usize,
}

/// Checks every file of `files` (the export's directory, by path) and returns what it holds, or why not.
pub fn verify(files: &BTreeMap<String, Vec<u8>>) -> Result<Verified> {
    let manifest = read_manifest(files)?;
    check_files(files, &manifest)?;
    check_versions(files, &manifest)?;
    let decisions: Vec<DecisionRecord> =
        read_list(files, DECISIONS_PATH)?.context("decisions.json is not in the export")?;
    if let Some(wrong) = decisions.iter().find(|record| record.org != manifest.org) {
        bail!("decisions.json holds a decision of org {}", wrong.org);
    }
    let numbers: Vec<Observation> = read_list(files, OUTCOMES_PATH)?.unwrap_or_default();
    Ok(Verified {
        manifest,
        decisions: decisions.len(),
        numbers: numbers.len(),
    })
}

fn read_manifest(files: &BTreeMap<String, Vec<u8>>) -> Result<Manifest> {
    let bytes = files
        .get(MANIFEST_PATH)
        .with_context(|| format!("{MANIFEST_PATH} is not in the export"))?;
    let value: Value = serde_json::from_slice(bytes).context("manifest.json is not JSON")?;
    let format = value.get("format").and_then(Value::as_u64);
    match format {
        Some(FORMAT) => {}
        Some(newer) if newer > FORMAT => {
            bail!("the export is of format {newer}, made by a newer node than this one reads")
        }
        _ => bail!("manifest.json has no format this node reads"),
    }
    serde_json::from_value(value).context("manifest.json is malformed")
}

/// Every file the manifest lists is there with its size and SHA-256, and no other file is.
fn check_files(files: &BTreeMap<String, Vec<u8>>, manifest: &Manifest) -> Result<()> {
    let mut listed = BTreeSet::new();
    for entry in &manifest.files {
        if !listed.insert(entry.path.as_str()) {
            bail!("manifest.json lists {} twice", entry.path);
        }
        let bytes = files
            .get(&entry.path)
            .ok_or_else(|| anyhow!("{} is listed and is not in the export", entry.path))?;
        if bytes.len() as u64 != entry.bytes || sha256_hex(bytes) != entry.sha256 {
            bail!("{} is not the file that was exported", entry.path);
        }
    }
    if let Some(extra) = files
        .keys()
        .find(|path| path.as_str() != MANIFEST_PATH && !listed.contains(path.as_str()))
    {
        bail!("{extra} is in the export and not in manifest.json");
    }
    Ok(())
}

/// The versions are the ones the manifest names, in order, and each is signed as the protocol says.
fn check_versions(files: &BTreeMap<String, Vec<u8>>, manifest: &Manifest) -> Result<()> {
    if manifest.first_version == 0 || manifest.first_version > manifest.last_version {
        bail!("manifest.json names no versions");
    }
    let mut chain: Vec<SignedOrg> = Vec::new();
    for version in manifest.first_version..=manifest.last_version {
        let path = version_path(version);
        let bytes = files
            .get(&path)
            .ok_or_else(|| anyhow!("{path} is not in the export"))?;
        let doc: SignedOrg = serde_json::from_slice(bytes)
            .with_context(|| format!("{path} is not an org version"))?;
        if doc.body.version != version || doc.body.org != manifest.org {
            bail!(
                "{path} holds version {} of org {}",
                doc.body.version,
                doc.body.org
            );
        }
        chain.push(doc);
    }
    check_chain(&chain)?;
    let last = chain.last().context("no version")?;
    if last.body.root.public_key.fingerprint() != manifest.root_fingerprint {
        bail!("the root key of the last version is not the one manifest.json names");
    }
    let versions_listed = files
        .keys()
        .filter(|path| path.starts_with("versions/"))
        .count();
    if versions_listed != chain.len() {
        bail!("the export holds a file under versions/ that is not a version in order");
    }
    Ok(())
}

/// Version 1 signed by the root key and well formed, or, for an export that starts later (a member's node keeps
/// the versions from the one it joined at), the first kept version signed by its own root key; then each next
/// version signed as the one before it allows.
fn check_chain(chain: &[SignedOrg]) -> Result<()> {
    let first = chain.first().context("no version")?;
    if first.body.version == 1 {
        verify_genesis(first).map_err(|error| anyhow!("version 1: {error}"))?;
    } else if first.signer != first.body.root.account
        || !first.verify(Kind::Org, &first.body.root.public_key)
    {
        bail!(
            "the signature of version {} does not match its root key",
            first.body.version
        );
    }
    for pair in chain.windows(2) {
        verify_successor(&pair[0], &pair[1])
            .map_err(|error| anyhow!("version {}: {error}", pair[1].body.version))?;
    }
    Ok(())
}

/// A file that holds a list of `T`, or `None` when the export has no such file.
fn read_list<T: serde::de::DeserializeOwned>(
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<Option<Vec<T>>> {
    let Some(bytes) = files.get(path) else {
        return Ok(None);
    };
    serde_json::from_slice(bytes)
        .map(Some)
        .with_context(|| format!("{path} cannot be read"))
}
