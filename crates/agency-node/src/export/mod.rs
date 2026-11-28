//! `agency export`: an org's records, out of this node, in files that verify on their own (docs/protocol.md,
//! "Exporting an org's records"). The export is made from what this node keeps; it never reads a key.
mod cli;
mod collect;
mod disk;
mod manifest;
mod verify;

pub use cli::{run, verify_dir, ExportArgs};
pub use manifest::{Manifest, MANIFEST_PATH};
pub use verify::Verified;

use agency_core::about::NODE_VERSION;
use agency_core::Node;
use agency_proto::Id;
use anyhow::{Context, Result};
use manifest::{ExportFile, Facts};
use std::path::Path;

/// The files of an export of `org`, the manifest last. Each part of what an export holds is added here, in
/// `collect`.
pub fn build(node: &Node, org: &Id) -> Result<Vec<ExportFile>> {
    let chain = collect::kept_versions(node, org)?;
    let (first, last) = (
        chain.first().context("no version of the org")?,
        chain.last().context("no version of the org")?,
    );
    let facts = Facts {
        org: org.clone(),
        release: NODE_VERSION.to_owned(),
        exported_at: node.now_text(),
        root_fingerprint: last.body.root.public_key.fingerprint(),
        first_version: first.body.version,
        last_version: last.body.version,
    };
    let mut files = collect::version_files(&chain)?;
    files.push(collect::decisions(node, org)?);
    files.extend(collect::outcomes(node, org)?);
    files.push(ExportFile::json(
        MANIFEST_PATH,
        &Manifest::describing(facts, &files),
    )?);
    Ok(files)
}

/// Writes the export of `org` to `to`, a directory that is absent or empty, and returns how many files it has.
pub fn export_to(node: &Node, org: &Id, to: &Path) -> Result<usize> {
    let files = build(node, org)?;
    disk::write(&files, to)?;
    Ok(files.len())
}

/// Checks the export in `dir`, which needs no node and no key.
pub fn verify_path(dir: &Path) -> Result<Verified> {
    verify::verify(&disk::read(dir)?)
}
