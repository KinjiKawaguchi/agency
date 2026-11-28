use super::*;
use crate::memory::ManualClock;
use agency_proto::clock::Duration as Span;
use agency_proto::metrics::SeriesKind::{Call, Client, Serve};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const NOW: &str = "2026-11-24T11:30:00Z";

fn recorder(dir: &Path, clock: &ManualClock) -> Recorder {
    Recorder::new(dir.join("metrics"), Arc::new(clock.clone()))
}

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

fn usage_route(status: &str) -> SeriesKey {
    SeriesKey::new(Serve, "GET /api/orgs/{org}/teams/{team}/usage", status)
}

fn one_series(metrics: &Metrics) -> &Series {
    assert_eq!(metrics.series.len(), 1, "{:?}", metrics.series);
    &metrics.series[0]
}

mod persistence;
mod recording;
