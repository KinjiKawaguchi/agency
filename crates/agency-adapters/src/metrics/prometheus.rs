//! The measurements since the node started, as the text a Prometheus scrape reads. Cumulative, as that format
//! wants; the files and `GET /api/metrics` are what keeps history.

use super::{Counts, SeriesKey};
use agency_proto::metrics::{SeriesKind, BUCKET_BOUNDS_MS};
use std::fmt::Write;

fn kind_word(kind: SeriesKind) -> &'static str {
    match kind {
        SeriesKind::Serve => "serve",
        SeriesKind::Call => "call",
        SeriesKind::Client => "client",
    }
}

/// A label value as the text format writes it: backslash, double quote, and line feed are escaped.
fn escaped(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// The labels of a series, in a fixed order: `kind`, `name`, `status`, then `peer` and `client` where it has them.
fn labels(key: &SeriesKey) -> String {
    let mut text = format!(
        "kind=\"{}\",name=\"{}\",status=\"{}\"",
        kind_word(key.kind),
        escaped(&key.name),
        escaped(&key.status)
    );
    if let Some(peer) = &key.peer {
        let _ = write!(text, ",peer=\"{}\"", escaped(peer));
    }
    if let Some(client) = &key.client {
        let _ = write!(text, ",client=\"{}\"", escaped(client));
    }
    text
}

/// The text of `totals`: a histogram of the durations in milliseconds, with cumulative buckets, and a counter of the errors.
pub fn text(totals: &[(SeriesKey, Counts)]) -> String {
    let mut out = String::new();
    out.push_str("# HELP agency_duration_milliseconds How long the node took to answer or to call, by series.\n");
    out.push_str("# TYPE agency_duration_milliseconds histogram\n");
    for (key, counts) in totals {
        let labels = labels(key);
        let mut cumulative: u64 = 0;
        for (index, bound) in BUCKET_BOUNDS_MS.iter().enumerate() {
            cumulative = cumulative.saturating_add(counts.buckets[index]);
            let _ = writeln!(
                out,
                "agency_duration_milliseconds_bucket{{{labels},le=\"{bound}\"}} {cumulative}"
            );
        }
        let _ = writeln!(
            out,
            "agency_duration_milliseconds_bucket{{{labels},le=\"+Inf\"}} {}",
            counts.count
        );
        let _ = writeln!(
            out,
            "agency_duration_milliseconds_sum{{{labels}}} {}",
            counts.sum_ms
        );
        let _ = writeln!(
            out,
            "agency_duration_milliseconds_count{{{labels}}} {}",
            counts.count
        );
    }
    out.push_str("# HELP agency_errors_total How many of the calls were errors, by series.\n");
    out.push_str("# TYPE agency_errors_total counter\n");
    for (key, counts) in totals {
        let _ = writeln!(
            out,
            "agency_errors_total{{{}}} {}",
            labels(key),
            counts.errors
        );
    }
    out
}

#[cfg(test)]
mod tests;
