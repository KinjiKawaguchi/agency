//! `agency org-create` and `root-rotate`: an org made on this node, whose account becomes its root key, and
//! the move of an org to a new root key.
use agency_core::{orgs, Node};
use anyhow::Result;
use clap::Args;

/// Creates an org (this node's account becomes its root key)
#[derive(Args)]
pub struct OrgCreate {
    #[arg(long)]
    org: String,
    #[arg(long)]
    label: String,
}

/// Moves an org to a new root key (on the root key's node): the next version names it, signed by the
/// current root key; the old key is forgotten once it is published
#[derive(Args)]
pub struct RootRotate {
    #[arg(long)]
    org: String,
}

pub fn org_create(node: &Node, args: OrgCreate) -> Result<()> {
    let doc = orgs::create(node, crate::id(&args.org)?, args.label)?;
    println!(
        "Created org {}. Root key fingerprint: {}",
        doc.body.org,
        node.key().public().fingerprint()
    );
    Ok(())
}

pub fn root_rotate(node: &Node, args: RootRotate) -> Result<()> {
    let org = args.org;
    let doc = orgs::rotate_root(node, &crate::id(&org)?)?;
    println!(
        "Org {org} has a new root key, fingerprint {} (version {})",
        doc.body.root.public_key.fingerprint(),
        doc.body.version
    );
    Ok(())
}
