//! `agency invite` and `join`: a link to an org made on the root key's node, and a node that joins by it.
use crate::id;
use agency_core::invites::{self, join, Joined};
use agency_core::Node;
use agency_proto::Role;
use anyhow::{bail, Result};
use clap::Args;
use std::io::Write;

/// Creates an invite link to an org (on the root key's node)
#[derive(Args)]
pub struct Invite {
    #[arg(long)]
    org: String,
    #[arg(long)]
    department: Option<String>,
    /// A seat of the org, so that whoever joins takes it (with its department); a claim link
    #[arg(long, conflicts_with = "department")]
    seat: Option<String>,
    /// member (reads and answers) or answerer (only answers)
    #[arg(long, default_value = "member")]
    role: String,
}

/// Joins an org from an invite link
#[derive(Args)]
pub struct Join {
    link: String,
    /// Joins without asking for confirmation
    #[arg(long)]
    yes: bool,
}

pub fn invite(node: &Node, args: Invite) -> Result<()> {
    let role = match args.role.as_str() {
        "member" => Role::Member,
        "answerer" => Role::Answerer,
        _ => bail!("--role must be member or answerer"),
    };
    let link = match args.seat.as_deref() {
        Some(seat) => invites::invite_for_seat(node, &id(&args.org)?, id(seat)?, role)?,
        None => {
            let department = args.department.as_deref().map(id).transpose()?;
            invites::invite(node, &id(&args.org)?, department, role)?
        }
    };
    println!("{link}");
    Ok(())
}

pub async fn join_org(node: &Node, args: Join) -> Result<()> {
    let yes = args.yes;
    match join(node, &args.link, |definition| yes || confirm_join(definition)).await? {
        Joined::Member(doc) => println!(
            "Joined org {} ({}) (version {})",
            doc.body.org,
            agency_proto::org::printable(&doc.body.label),
            doc.body.version
        ),
        Joined::Waiting(message) => println!(
            "The org's root key's node holds the join for its owner. {message} This node asks again on its own, and with `agency sync`."
        ),
    }
    Ok(())
}

fn confirm_join(definition: &agency_proto::OrgDefinition) -> bool {
    print!(
        "Join the org \"{}\" ({})? The fingerprint of root key {} matches the link. [y/N] ",
        agency_proto::org::printable(&definition.label),
        definition.org,
        definition.root.account
    );
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
}
