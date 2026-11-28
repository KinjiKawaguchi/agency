//! `agency team-add`, `team-set`, `team-move` and `team-remove`: a team's place in the org's definition,
//! changed on the root key's node. Which department a team is in is `team-dept`.
use crate::absolute;
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::{orgs, peer_about, teams, Node};
use agency_proto::org::{is_node_origin, TeamPlace};
use agency_proto::Id;
use anyhow::{bail, Result};
use clap::Args;
use std::path::PathBuf;

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

/// Adds a team to an org (on the root key's node). Without --node, the team runs on this node
#[derive(Args)]
pub struct TeamAdd {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    #[arg(long)]
    label: String,
    /// For a team on this node: the scrum harness state directory. The team runtime is `<scrum> runtime`.
    #[arg(long)]
    state_dir: Option<PathBuf>,
    /// For a team on this node: the org person who owns it (default: this node's account)
    #[arg(long)]
    owner: Option<String>,
    #[arg(long)]
    node: Option<String>,
    /// The department the team is in
    #[arg(long)]
    department: Option<String>,
}

/// Moves a team's place in the org to another node origin (on the root key's node), for example after
/// this node's URL changed
#[derive(Args)]
pub struct TeamMove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    /// The node origin other nodes reach the team at (http(s)://host[:port])
    #[arg(long)]
    node: String,
}

/// What `team-move` says after it moved a team: the team's node takes no decision made before the move, unless the
/// move only spelled the same origin another way or renamed this node's own origin.
const TEAM_MOVE_NOTE: &str = "Decisions made for this team before this version and not yet delivered are not taken by its node now: \
whoever made one is told it was refused, and makes it again. A version that only writes the same origin another way (the host's case, \
the default port), or that renames this node's own origin to one it was configured with, changes nothing.";

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

pub async fn team_add(node: &Node, args: TeamAdd) -> Result<()> {
    let (org, team) = (id(&args.org)?, id(&args.team)?);
    let place_url = args.node.unwrap_or_else(|| node.url());
    if !is_node_origin(&place_url) {
        bail!("--node must be http(s)://host[:port]");
    }
    let is_here = place_url == node.url();
    if is_here && args.state_dir.is_none() {
        bail!("a team on this node needs --state-dir");
    }
    if !is_here {
        peer_about::read_origin(node, &org, &place_url).await?;
    }
    let doc = orgs::add_team(
        node,
        &org,
        TeamPlace::new(team.clone(), args.label, place_url)
            .with_department(args.department.as_deref().map(id).transpose()?),
    )?;
    if let (true, Some(state_dir)) = (is_here, args.state_dir) {
        let owner = args
            .owner
            .as_deref()
            .map(id)
            .transpose()?
            .unwrap_or_else(|| node.account());
        let runtime = RuntimeCommand::scrum(&node.config().scrum, &absolute(&state_dir)?);
        node.update_config(|config| {
            config
                .teams
                .push(HostedTeam::new(org.clone(), team.clone(), owner, runtime))
        })?;
    }
    println!(
        "Added team {team} to org {org} (version {})",
        doc.body.version
    );
    Ok(())
}

pub async fn team_move(node: &Node, args: TeamMove) -> Result<()> {
    let TeamMove {
        org,
        team,
        node: origin,
    } = args;
    if origin != node.url() {
        peer_about::read_origin(node, &id(&org)?, &origin).await?;
    }
    let doc = orgs::move_team(node, &id(&org)?, &id(&team)?, origin.clone())?;
    println!(
        "Moved team {team} of org {org} to {origin} (version {})",
        doc.body.version
    );
    println!("{TEAM_MOVE_NOTE}");
    Ok(())
}
