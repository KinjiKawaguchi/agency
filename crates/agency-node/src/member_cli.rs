//! `agency member-remove`, `agency guest-remove` and `agency guests`: the root's node signs a version of the org
//! without a member, or with a hosted member turned back into a seat, and lists the members it hosts.
use agency_core::{members, Node};
use agency_proto::Id;
use anyhow::{bail, Result};
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

/// Ends a hosted member's access (on the root key's node): its devices' keys are revoked, its sign-in, push registrations
/// and the list of requests it was told of are dropped, and it leaves the org in one version. With --to-seat a seat of
/// the member's id stands in its place (and one for each seat it took), so the requests addressed to it stay addressed
/// to someone: answered by the members named with --answered-by, or by the team's owner
#[derive(Args)]
pub struct GuestRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    member: String,
    /// Put a seat in the member's place, with its label, department and what to ask about
    #[arg(long)]
    to_seat: bool,
    /// A member who answers for the seat, in order of preference; repeat for more (with --to-seat)
    #[arg(long, requires = "to_seat")]
    answered_by: Vec<String>,
}

pub fn guest_remove(node: &Node, args: GuestRemove) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let member = Id::parse(&args.member).map_err(anyhow::Error::msg)?;
    let doc = node
        .store()
        .load_org(&org)
        .ok_or_else(|| anyhow::anyhow!("no org {org}"))?;
    if !doc
        .body
        .member(&member)
        .is_some_and(|person| person.is_hosted())
    {
        bail!("{member} is not a hosted member of {org}; `agency member-remove` removes any other member");
    }
    let removed = if args.to_seat {
        let answered_by = args
            .answered_by
            .iter()
            .map(|raw| Id::parse(raw).map_err(anyhow::Error::msg))
            .collect::<Result<Vec<_>>>()?;
        members::remove_member_to_seat(node, &org, &member, &answered_by)?
    } else {
        members::remove_member(node, &org, &member)?
    };
    let now = if args.to_seat {
        "; a seat of its id stands in its place"
    } else {
        ""
    };
    println!(
        "Ended {member}'s access in org {org} (version {}); its devices' keys are revoked{now}",
        removed.body.version
    );
    Ok(())
}

/// Lists the members this node hosts in an org, with how many devices each has (no email)
#[derive(Args)]
pub struct Guests {
    #[arg(long)]
    org: String,
}

pub fn guests(node: &Node, args: Guests) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let doc = node
        .store()
        .load_org(&org)
        .ok_or_else(|| anyhow::anyhow!("no org {org}"))?;
    for person in doc.body.people.iter().filter(|person| person.is_hosted()) {
        println!("{} ({} devices)", person.id, person.devices.len());
    }
    Ok(())
}
