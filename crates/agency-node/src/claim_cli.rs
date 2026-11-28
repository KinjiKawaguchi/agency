//! `agency claims`, `claim-confirm` and `claim-reject`: the claims of seats that a person's browser redeemed and that
//! wait for the owner, who compares the pairing code on the person's screen with the one here (on the root key's
//! node, at its terminal, where the root key's presence prompt, if it asks, is the owner's own).
use agency_core::claims;
use agency_core::Node;
use agency_proto::org::printable;
use agency_proto::Id;
use anyhow::Result;
use clap::Args;

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

/// Confirms a claim after you compare its pairing code: the seat becomes a member this node hosts
#[derive(Args)]
pub struct ClaimConfirm {
    #[arg(long)]
    org: String,
    #[arg(long)]
    seat: String,
    /// The pairing code the person's screen shows (`agency claims` lists the one the node holds)
    #[arg(long)]
    pairing: String,
}

/// Refuses a claim: it is dropped, and the owner makes a new link if the person should try again
#[derive(Args)]
pub struct ClaimReject {
    #[arg(long)]
    org: String,
    #[arg(long)]
    seat: String,
}

/// The lines that list the claims waiting for the owner, and none when there is none. Texts someone else chose (a
/// browser's name, a seat's label) are printed without what hides or reorders a line.
pub fn claim_lines(node: &Node) -> Vec<String> {
    claims::pending(node)
        .into_iter()
        .map(|claim| {
            format!(
                "{}/{} ({}): {}, pairing code {}, since {}",
                claim.org,
                claim.seat,
                printable(&claim.seat_label),
                printable(&claim.device_label),
                claim.pairing_code,
                claim.redeemed_at
            )
        })
        .collect()
}

pub fn list(node: &Node) {
    let lines = claim_lines(node);
    if lines.is_empty() {
        println!("No claim waits for you.");
    }
    for line in lines {
        println!("{line}");
    }
}

pub async fn confirm(node: &Node, args: ClaimConfirm) -> Result<()> {
    let doc = claims::confirm(node, &id(&args.org)?, &id(&args.seat)?, &args.pairing).await?;
    println!(
        "Seat {} of {} is now a member this node hosts (version {}).",
        args.seat, args.org, doc.body.version
    );
    Ok(())
}

pub async fn reject(node: &Node, args: ClaimReject) -> Result<()> {
    claims::reject(node, &id(&args.org)?, &id(&args.seat)?).await?;
    println!(
        "The claim of seat {} was dropped. Make a new link with `agency seat-invite` if the person should try again.",
        args.seat
    );
    Ok(())
}
