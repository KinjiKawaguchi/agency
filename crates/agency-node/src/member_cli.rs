//! `agency member-remove`: the root's node signs a version of the org without a member.
use agency_core::ports::{ConfigStore, GuestStore};
use agency_core::{members, Node};
use agency_proto::Id;
use anyhow::Result;
use clap::Args;

/// Removes a member from an org, with its devices' keys revoked and this node's sign-in for it dropped (on the root
/// key's node). Refused while a seat, an external party, a department or a team on this node still names it
#[derive(Args)]
pub struct MemberRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    member: String,
}

pub fn member_remove(node: &Node, args: MemberRemove) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let member = Id::parse(&args.member).map_err(anyhow::Error::msg)?;
    let doc = members::remove_member(node, &org, &member)?;
    println!(
        "Removed {member} from org {org} (version {}); its devices' keys are revoked",
        doc.body.version
    );
    Ok(())
}

/// Removes a hosted member and their Access sign-in records (on the root key's node).
#[derive(Args)]
pub struct GuestRemove {
    #[arg(long)]
    pub org: String,
    #[arg(long)]
    pub member: String,
}

pub fn guest_remove(node: &Node, args: GuestRemove) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let member = Id::parse(&args.member).map_err(anyhow::Error::msg)?;
    let guest = node
        .store()
        .guests()
        .into_iter()
        .any(|one| one.org == org && one.member == member);
    if !guest {
        anyhow::bail!("{member} is not a hosted member of {org}");
    }
    let doc = members::remove_member(node, &org, &member)?;
    println!(
        "Removed hosted member {member} from org {org} (version {})",
        doc.body.version
    );
    Ok(())
}

#[derive(Args)]
pub struct Guests {
    #[arg(long)]
    pub org: String,
}

pub fn guests(node: &Node, args: Guests) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let doc = node
        .store()
        .load_org(&org)
        .ok_or_else(|| anyhow::anyhow!("no org {org}"))?;
    for guest in node
        .store()
        .guests()
        .into_iter()
        .filter(|one| one.org == org)
    {
        let devices = doc
            .body
            .member(&guest.member)
            .map_or(0, |member| member.devices.len());
        println!("{} ({devices} devices)", guest.member);
    }
    Ok(())
}
