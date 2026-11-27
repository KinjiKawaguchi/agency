//! `agency outcome-add` and `outcome-list`: the owner enters a number for a KPI and reads one KPI's history,
//! on the node's terminal. The numbers are business data kept only on this node; these commands go through
//! the same use cases as the owner-only routes, so the limits are the same.
use agency_core::{outcomes, Node};
use agency_proto::observation::{Observation, ObservationInput, Source};
use agency_proto::org::printable;
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

/// One number of the history as a line: where it came from and its note are text someone else wrote, printed as such.
fn entry_line(one: Observation) -> String {
    let source = match one.source {
        Source::Manual => "by hand".to_owned(),
        Source::Connector { name } => format!("from {}", printable(&name)),
    };
    let note = one
        .note
        .map_or_else(String::new, |note| format!(" ({})", printable(&note)));
    format!(
        "{}  {}  entered {} by {} {source}{note}",
        one.date.as_str(),
        one.value,
        one.entered_at,
        one.entered_by
    )
}

pub fn outcome_list(node: &Node, args: OutcomeList) -> Result<()> {
    let entries = outcomes::history(node, &id(&args.org)?, &id(&args.kpi)?, args.limit)?;
    for one in entries {
        println!("{}", entry_line(one));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::Timestamp;

    fn entry(source: Source, note: Option<&str>) -> Observation {
        Observation {
            kpi: Id::parse("revenue").unwrap(),
            date: Date::parse("2026-11-24").unwrap(),
            value: 120.0,
            source,
            entered_at: Timestamp::parse("2026-11-25T09:00:00Z").unwrap(),
            entered_by: Id::parse("alice").unwrap(),
            note: note.map(str::to_owned),
        }
    }

    #[test]
    fn a_number_says_where_it_came_from_and_its_note() {
        assert_eq!(
            entry_line(entry(Source::Manual, Some("restated"))),
            "2026-11-24  120  entered 2026-11-25T09:00:00Z by alice by hand (restated)"
        );
        assert_eq!(
            entry_line(entry(Source::Manual, None)),
            "2026-11-24  120  entered 2026-11-25T09:00:00Z by alice by hand"
        );
    }

    #[test]
    fn what_someone_else_wrote_is_printed_on_one_line_without_what_hides_or_reorders_it() {
        let line = entry_line(entry(
            Source::Connector {
                name: "bi\u{202E}lling\n[signed]".into(),
            },
            Some("see\u{202E}this\tnote\nsecond line\u{1b}[2J"),
        ));
        // The history is one line for each number: a line break or a tab in a name or a note is a mark, so that none forges a second record.
        assert!(line.contains("from bi?lling?[signed]"), "{line}");
        assert!(line.contains("(see?this?note?second line?[2J)"), "{line}");
        assert!(
            !line.contains('\u{202E}')
                && !line.contains('\u{1b}')
                && !line.contains('\n')
                && !line.contains('\t')
        );
    }
}
