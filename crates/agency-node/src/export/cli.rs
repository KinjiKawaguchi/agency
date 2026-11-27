//! The `agency export` command.
use super::{export_to, verify_path};
use agency_core::Node;
use agency_proto::Id;
use anyhow::{Context, Result};
use clap::Args;
use std::path::{Path, PathBuf};

/// Writes an org's records out of this node, or checks such an export
#[derive(Args)]
pub struct ExportArgs {
    /// The org to export: every signed version this node kept, this node's decision log for it, and, on the
    /// owner's node, the numbers entered for it. No key and no secret is written
    #[arg(long, requires = "to", conflicts_with = "verify")]
    org: Option<String>,
    /// Where to write it: a directory that is absent or empty
    #[arg(long, requires = "org", conflicts_with = "verify")]
    to: Option<PathBuf>,
    /// Checks the export in this directory: the manifest against the files, and the chain of the org's versions.
    /// Needs no node and no key
    #[arg(long, value_name = "DIR", conflicts_with_all = ["org", "to"])]
    verify: Option<PathBuf>,
}

impl ExportArgs {
    /// The directory to check, when the command only checks one (it needs no node).
    pub fn only_verifies(&self) -> Option<&PathBuf> {
        self.verify.as_ref()
    }
}

/// `agency export --verify <dir>`, which reads no node.
pub fn verify_dir(dir: &Path) -> Result<()> {
    let verified =
        verify_path(dir).with_context(|| format!("{} does not verify", dir.display()))?;
    let manifest = &verified.manifest;
    println!(
        "{} verifies: org {}, versions {} to {} signed as the protocol says, {} decisions, {} numbers.",
        dir.display(),
        manifest.org,
        manifest.first_version,
        manifest.last_version,
        verified.decisions,
        verified.numbers
    );
    println!(
        "Root key fingerprint {}: compare it with the one you know this org by; the manifest is not signed.",
        manifest.root_fingerprint
    );
    Ok(())
}

/// `agency export --org <org> --to <dir>`.
pub fn run(node: &Node, args: ExportArgs) -> Result<()> {
    let (Some(org), Some(to)) = (args.org, args.to) else {
        anyhow::bail!("give --org and --to, or --verify");
    };
    let org = Id::parse(&org).map_err(anyhow::Error::msg)?;
    let count = export_to(node, &org, &to)?;
    println!(
        "Exported org {org} to {}: {count} files. `agency export --verify {}` checks them.",
        to.display(),
        to.display()
    );
    Ok(())
}
