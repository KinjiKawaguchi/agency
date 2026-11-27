//! `agency audit`: an org's change history in words, from the signed versions this node keeps.
use agency_core::audit::{org_history, HistoryEntry};
use agency_core::Node;
use agency_proto::{clock, Id};
use anyhow::{bail, Result};
use clap::Args;

/// Lists every version of an org this node keeps, oldest first: when it was signed, by which key, and what it
/// changed. A version this node does not keep is listed as such
#[derive(Args)]
pub struct Audit {
    #[arg(long)]
    org: String,
    /// Only the versions signed at or after this time (RFC 3339, such as 2026-11-01T00:00:00Z)
    #[arg(long)]
    since: Option<String>,
    /// One JSON object per version, one per line
    #[arg(long)]
    json: bool,
}

fn line(entry: &HistoryEntry) -> String {
    if !entry.is_kept {
        return format!("version {}: not kept on this node", entry.version);
    }
    let changes = match entry.changes.is_empty() {
        true => "nothing else changes".to_owned(),
        false => entry.changes.join("; "),
    };
    format!(
        "version {} at {}, signed by {}: {changes}",
        entry.version,
        entry.at.as_deref().unwrap_or("?"),
        entry
            .signed_by
            .as_deref()
            .unwrap_or("no key of the version before")
    )
}

pub fn audit(node: &Node, args: Audit) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let since = match args.since.as_deref() {
        None => None,
        Some(raw) => match clock::parse(raw) {
            Some(at) => Some(at),
            None => bail!("--since must be an RFC 3339 time, such as 2026-11-01T00:00:00Z"),
        },
    };
    for entry in org_history(node, &org, since)? {
        if args.json {
            println!("{}", serde_json::to_string(&entry)?);
        } else {
            println!("{}", line(&entry));
        }
    }
    Ok(())
}
