//! `agency decide`: an answer to a request, signed with this terminal's device.
use crate::device_cli::CliDevice;
use crate::id;
use agency_adapters::files::DataDir;
use agency_core::decisions::Decided;
use agency_core::signed_decisions::decide_with_device;
use agency_core::Node;
use agency_proto::{EnvelopeKind, Payload};
use anyhow::{Context, Result};
use clap::Args;

/// Answers a request, signed with the CLI device (applied directly to a team on this node; delivered to a team on another node)
#[derive(Args)]
pub struct Decide {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    /// The request id
    #[arg(long)]
    request: String,
    #[arg(long)]
    choice: Option<String>,
    /// The answer text (text starting with - is taken as is)
    #[arg(long, default_value = "", allow_hyphen_values = true)]
    text: String,
}

pub async fn decide(node: &Node, data: &DataDir, args: Decide) -> Result<()> {
    let device = CliDevice::load(data)?;
    let outcome = decide_with_device(
        node,
        device.signer(),
        &id(&args.org)?,
        &id(&args.team)?,
        EnvelopeKind::Answer,
        args.request,
        Payload {
            text: args.text,
            choice: args.choice,
        },
    )
    .await
    .context("is the CLI device added in this org? `agency status` lists your devices")?;
    println!(
        "{}",
        match outcome {
            Decided::Applied => "Applied to the team on this node".to_owned(),
            Decided::Delivered => "Delivered to the team's node".to_owned(),
            Decided::Queued(why) =>
                format!("The team's node is unreachable; the decision waits in the outbox ({why})"),
            Decided::Refused(why) => format!("The team's node refused: {why}"),
        }
    );
    Ok(())
}
