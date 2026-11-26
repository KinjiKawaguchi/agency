//! `agency team-set` and `team-remove`: a team's label, and its place in the org's definition, changed
//! on the root key's node. Which department a team is in is `team-dept`, and where it runs `team-move`.
use agency_core::{teams, Node};
use agency_proto::Id;
use anyhow::Result;
use clap::Args;

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

/// Renames a team in the org's definition (on the root key's node)
#[derive(Args)]
pub struct TeamSet {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    /// The team's new label
    #[arg(long)]
    label: String,
}

/// Removes a team from the org's definition (on the root key's node). Refuses while the team is still
/// in this node's config.json
#[derive(Args)]
pub struct TeamRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
}

pub fn team_set(node: &Node, args: TeamSet) -> Result<()> {
    let doc = teams::set_label(node, &id(&args.org)?, &id(&args.team)?, args.label.clone())?;
    println!(
        "Renamed team {} of org {} to {} (version {})",
        args.team, args.org, args.label, doc.body.version
    );
    Ok(())
}

pub fn team_remove(node: &Node, args: TeamRemove) -> Result<()> {
    let doc = teams::remove(node, &id(&args.org)?, &id(&args.team)?)?;
    println!(
        "Removed team {} from org {} (version {})",
        args.team, args.org, doc.body.version
    );
    Ok(())
}
