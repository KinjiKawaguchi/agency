//! `agency metrics`: the node's own measurements (docs/protocol.md, "Metrics") as a table, read from the files
//! in the data directory, so it works while the node is stopped and needs no key. A running node writes its
//! numbers once a minute, so the last minute is not in it; `GET /api/metrics` has it.
use agency_adapters::files::DataDir;
use agency_adapters::metrics::Recorder;
use agency_adapters::system::SystemClock;
use agency_core::ports::Clock;
use agency_proto::metrics::{
    valid_metrics_days, Metrics, Series, SeriesKind, IN_NODE_READ_P95_BUDGET_MS, MAX_METRICS_DAYS,
};
use anyhow::{bail, Result};
use clap::Args;
use std::io::Write;
use std::sync::Arc;

#[derive(Args)]
pub struct MetricsArgs {
    /// How many days back, 1 to 30 (default 1)
    #[arg(long, default_value_t = 1)]
    days: u32,
    /// Only this kind: `serve` (a route the node answers), `call` (a call it makes), or `client`
    #[arg(long)]
    kind: Option<String>,
    /// Only the series of this exact name, such as `GET /api/me` or `runtime view`
    #[arg(long)]
    name: Option<String>,
    /// Print the answer as JSON, as `GET /api/metrics` gives it
    #[arg(long)]
    json: bool,
}

fn kind_of(text: &str) -> Result<SeriesKind> {
    match text {
        "serve" => Ok(SeriesKind::Serve),
        "call" => Ok(SeriesKind::Call),
        "client" => Ok(SeriesKind::Client),
        _ => bail!("--kind is serve, call, or client; not \"{text}\""),
    }
}

fn kind_word(kind: SeriesKind) -> &'static str {
    match kind {
        SeriesKind::Serve => "serve",
        SeriesKind::Call => "call",
        SeriesKind::Client => "client",
    }
}

/// A percentile as the table writes it: at most that many milliseconds (it is the upper bound of its
/// bucket), or `>30000` for one slower than the last bound (a series has calls, so it has all three).
fn percentile(value: Option<u32>) -> String {
    match value {
        Some(millis) => format!("<={millis}"),
        None => ">30000".to_owned(),
    }
}

/// Whether the budget is kept: `-` where it does not apply, `ok` when the 95th percentile is within it, and
/// `OVER` when it is not (also for one slower than 30 s).
fn budget_column(series: &Series) -> &'static str {
    match (series.is_in_node_read, series.p95_ms) {
        (false, _) => "-",
        (true, Some(p95)) if p95 <= IN_NODE_READ_P95_BUDGET_MS => "ok",
        (true, _) => "OVER",
    }
}

fn row(series: &Series) -> Vec<String> {
    let name = match (&series.peer, &series.client) {
        (Some(peer), _) => format!("{} [{peer}]", series.name),
        (_, Some(client)) => format!("{} [{client}]", series.name),
        _ => series.name.clone(),
    };
    vec![
        kind_word(series.kind).to_owned(),
        name,
        series.status.clone(),
        series.count.to_string(),
        series.errors.to_string(),
        percentile(series.p50_ms),
        percentile(series.p95_ms),
        percentile(series.p99_ms),
        budget_column(series).to_owned(),
    ]
}

/// The table: a header and a row for each series, each column as wide as its widest cell, the numbers on the
/// right.
fn table(metrics: &Metrics) -> Vec<String> {
    let header = [
        "KIND", "NAME", "STATUS", "COUNT", "ERRORS", "P50", "P95", "P99", "BUDGET",
    ]
    .map(str::to_owned)
    .to_vec();
    let rows: Vec<Vec<String>> = std::iter::once(header)
        .chain(metrics.series.iter().map(row))
        .collect();
    let widths: Vec<usize> = (0..9)
        .map(|column| {
            rows.iter()
                .map(|r| r[column].chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let numbers = [3, 4, 5, 6, 7];
    rows.iter()
        .map(|cells| {
            let line: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(column, cell)| {
                    if numbers.contains(&column) {
                        format!("{cell:>width$}", width = widths[column])
                    } else {
                        format!("{cell:<width$}", width = widths[column])
                    }
                })
                .collect();
            line.join("  ").trim_end().to_owned()
        })
        .collect()
}

/// What to print: the answer as JSON, or the table and what it means.
fn lines(data: &DataDir, args: &MetricsArgs, clock: Arc<dyn Clock>) -> Result<Vec<String>> {
    if !valid_metrics_days(args.days) {
        bail!(
            "--days is a number from 1 to {MAX_METRICS_DAYS}; not {}",
            args.days
        );
    }
    let kind = args.kind.as_deref().map(kind_of).transpose()?;
    let recorder = Recorder::new(data.root().join("metrics"), clock);
    let metrics = recorder.read(args.days, kind, args.name.as_deref());
    if args.json {
        return Ok(vec![serde_json::to_string_pretty(&metrics)?]);
    }
    if metrics.series.is_empty() {
        return Ok(vec!["No measurements yet.".to_owned()]);
    }
    let mut out = table(&metrics);
    out.push(String::new());
    out.push(format!(
        "{} to {}, in milliseconds. A percentile is the upper bound of its bucket: <=100 means at most 100.",
        metrics.from.as_str(),
        metrics.until.as_str()
    ));
    out.push(format!(
        "BUDGET: ok or OVER for a read the node answers from its own state, held to a 95th percentile of at most {IN_NODE_READ_P95_BUDGET_MS} ms."
    ));
    Ok(out)
}

/// Prints it. A reader that stops early (`| head`) is not an error.
pub fn run(data: &DataDir, args: &MetricsArgs) -> Result<()> {
    let lines = lines(data, args, Arc::new(SystemClock))?;
    let mut out = std::io::stdout().lock();
    for line in lines {
        if writeln!(out, "{line}").is_err() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
