//! `agency outcome-add` and `outcome-list`: the owner enters a number for a KPI and reads one KPI's history,
//! on the node's terminal. The numbers are business data kept only on this node; these commands go through
//! the same use cases as the owner-only routes, so the limits are the same.
use agency_core::{outcomes, Node};
use agency_proto::observation::ObservationInput;
use agency_proto::{Date, Id};
use anyhow::Result;
use clap::Args;

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

/// Enters a number for a KPI on a day (kept only on this node)
#[derive(Args)]
pub struct OutcomeAdd {
    #[arg(long)]
    org: String,
    #[arg(long)]
    kpi: String,
    /// The day the number is as of (YYYY-MM-DD): the last day of a period for a monthly figure
    #[arg(long)]
    date: String,
    /// The number, in the unit of the KPI
    #[arg(long, allow_hyphen_values = true)]
    value: f64,
    #[arg(long)]
    note: Option<String>,
    /// The name of the program that produced the number; without it the number is entered by hand
    #[arg(long)]
    connector: Option<String>,
}

/// Lists one KPI's numbers, newest first, the ones a later entry replaced included
#[derive(Args)]
pub struct OutcomeList {
    #[arg(long)]
    org: String,
    #[arg(long)]
    kpi: String,
    /// How many entries, at most 500 (default 100)
    #[arg(long)]
    limit: Option<u32>,
}

pub async fn outcome_add(node: &Node, args: OutcomeAdd) -> Result<()> {
    let input = ObservationInput {
        kpi: id(&args.kpi)?,
        date: Date::parse(&args.date).map_err(anyhow::Error::msg)?,
        value: args.value,
        note: args.note,
        connector: args.connector,
    };
    let saved = outcomes::record(node, &id(&args.org)?, input).await?;
    println!(
        "Entered a number for KPI {} on {} in org {}",
        saved.kpi,
        saved.date.as_str(),
        args.org
    );
    Ok(())
}

pub fn outcome_list(node: &Node, args: OutcomeList) -> Result<()> {
    let entries = outcomes::history(node, &id(&args.org)?, &id(&args.kpi)?, args.limit)?;
    for one in entries {
        let source = match one.source {
            agency_proto::observation::Source::Manual => "by hand".to_owned(),
            agency_proto::observation::Source::Connector { name } => format!("from {name}"),
        };
        let note = one
            .note
            .map_or_else(String::new, |note| format!(" ({note})"));
        println!(
            "{}  {}  entered {} by {} {source}{note}",
            one.date.as_str(),
            one.value,
            one.entered_at,
            one.entered_by
        );
    }
    Ok(())
}
