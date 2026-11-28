//! `agency budget status`: this week's spending of a team and its org against their plans, as the Client API
//! answers it.
use agency_core::Node;
use agency_proto::Id;
use anyhow::Result;
use clap::{Args, Subcommand};

/// Shows this week's team and organization spending against their plans as JSON
#[derive(Args)]
pub struct Budget {
    #[command(subcommand)]
    command: BudgetCommand,
}

#[derive(Subcommand)]
enum BudgetCommand {
    /// Shows the current Monday-based week's spending and budget state as JSON
    Status {
        #[arg(long)]
        org: String,
        #[arg(long)]
        team: String,
    },
}

pub async fn run(node: &Node, args: Budget) -> Result<()> {
    match args.command {
        BudgetCommand::Status { org, team } => {
            print_budget_status(node, &crate::id(&org)?, &crate::id(&team)?).await
        }
    }
}

/// Prints the budget status, which the Client API answers too: an unavailable team's runtime makes the
/// report `partial` and does not prevent it.
async fn print_budget_status(node: &Node, org: &Id, team: &Id) -> Result<()> {
    let report = agency_adapters::budget::budget_status(node, org, team).await?;
    println!("{}", agency_node::budget::json_line(&report)?);
    Ok(())
}
