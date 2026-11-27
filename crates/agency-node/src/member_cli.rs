//! `agency member-remove`: the root's node signs a version of the org without a member.
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
