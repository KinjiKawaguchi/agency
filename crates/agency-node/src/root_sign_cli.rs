//! `agency root-sign`: the changes others asked the root to sign (joins, device changes), held because the
//! root key asks for the owner's presence. Every one is listed before the one prompt, and only those are
//! signed.
use agency_core::pending_root::{self, purpose_of, sign_held, PendingRootChange};
use agency_core::Node;
use agency_proto::Id;
use anyhow::Result;
use clap::Args;
use std::io::{BufRead, Write};

/// Lists the changes held for the root key's owner and signs them after one presence prompt; with
/// `--refuse`, drops one unsigned
#[derive(Args)]
pub struct RootSign {
    /// Only this org's changes
    #[arg(long)]
    org: Option<String>,
    /// Drops this held change instead (its id as listed)
    #[arg(long)]
    refuse: Option<String>,
    /// Signs without asking here first (the presence prompt still asks)
    #[arg(long)]
    yes: bool,
}

fn by_org(held: Vec<PendingRootChange>) -> Vec<(Id, Vec<PendingRootChange>)> {
    let mut groups: Vec<(Id, Vec<PendingRootChange>)> = vec![];
    for one in held {
        match groups.iter_mut().find(|(org, _)| org == &one.org) {
            Some((_, list)) => list.push(one),
            None => groups.push((one.org.clone(), vec![one])),
        }
    }
    groups
}

fn confirm(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer).is_ok()
        && answer.trim().eq_ignore_ascii_case("y")
}

pub async fn root_sign(node: &Node, args: RootSign) -> Result<()> {
    if let Some(id) = &args.refuse {
        pending_root::refuse(node, id)?;
        println!("Dropped {id}; it will not be signed.");
        return Ok(());
    }
    let only = args
        .org
        .as_deref()
        .map(Id::parse)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let held: Vec<PendingRootChange> = pending_root::waiting(node)?
        .into_iter()
        .filter(|one| only.as_ref().is_none_or(|org| &one.org == org))
        .collect();
    if held.is_empty() {
        println!("Nothing waits for the root key.");
        return Ok(());
    }
    for (org, list) in by_org(held) {
        println!("Org {org}:");
        for one in &list {
            println!("  {}  {}  (asked {})", one.id, one.what(), one.received_at);
        }
        if !args.yes && !confirm(&format!("{}?", purpose_of(&org, &list))) {
            println!("  Nothing signed for {org}.");
            continue;
        }
        let ids: Vec<String> = list.iter().map(|one| one.id.clone()).collect();
        let run = sign_held(node, &org, &ids).await?;
        for (one, doc) in &run.signed {
            println!("  Signed: {} (version {})", one.what(), doc.body.version);
        }
        for (one, why) in &run.dropped {
            println!("  Dropped: {} ({why})", one.what());
        }
    }
    Ok(())
}
