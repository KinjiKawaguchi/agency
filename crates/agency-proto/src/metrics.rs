//! How fast a node answers and how its calls go, measured by the node itself and read only by its owner
//! (`GET /api/metrics`, docs/protocol.md, "Metrics"). Nothing here leaves the node but through that route,
//! and nothing in it holds a request body, a token, an address, or a query string: a series is named by a
//! route template (`GET /api/orgs/{org}/teams/{team}/usage`), never by a path with ids in it.
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The upper bounds, in milliseconds, of the histogram buckets of a series. A duration falls in the first
/// bucket whose bound it does not pass; a longer one falls in one more bucket, the last, which has no bound.
pub const BUCKET_BOUNDS_MS: [u32; 14] = [
    1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10_000, 30_000,
];
/// How many buckets a series has: one for each bound, and the one for what is slower than the last bound.
pub const BUCKETS: usize = BUCKET_BOUNDS_MS.len() + 1;
/// The longest range a metrics read may cover, in days (the node keeps 30).
pub const MAX_METRICS_DAYS: u32 = 30;

/// What a series measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SeriesKind {
    /// A route this node answers (Client API or federation): `status` is the class of the HTTP status.
    Serve,
    /// A call this node makes (a peer, the team runtime, a push service, GitHub): `status` is `ok`,
    /// `error`, `timeout`, or, for a peer, the class of its HTTP status.
    Call,
    /// What a client observed and reported to its home node: `status` is the class, or `network`.
    Client,
}

/// One series: the calls of one kind, name, and status (and peer or client), as a histogram.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct Series {
    pub kind: SeriesKind,
    /// The route template (`GET /api/orgs/{org}/teams/{team}/usage`) or the call (`runtime view`,
    /// `peer get_view`, `apns send`, `github graphql`).
    pub name: String,
    /// `2xx`, `3xx`, `4xx`, `5xx`, `ok`, `error`, `timeout`, or `network`, as `kind` says.
    pub status: String,
    /// For a call to a peer: the host of the peer's origin, which stays on the node (an export leaves it
    /// out unless the owner asks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// For what a client reported: `web` or `ios`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[ts(type = "number")]
    pub count: u64,
    /// How many of `count` were errors: a 5xx, or a call that failed.
    #[ts(type = "number")]
    pub errors: u64,
    /// The durations added up, in milliseconds.
    #[ts(type = "number")]
    pub sum_ms: u64,
    /// How many durations fell in each bucket (`BUCKETS` of them, in the order of `BUCKET_BOUNDS_MS`, the
    /// last for what is slower than the last bound). They add up to `count`.
    #[ts(type = "Array<number>")]
    pub buckets: Vec<u64>,
    /// The median, the 95th, and the 99th percentile by nearest rank, each the **upper bound of its
    /// bucket**, in milliseconds: a duration is not known more exactly than its bucket. Left out when
    /// there is none to give or when it lies in the last bucket (slower than the last bound).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p50_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99_ms: Option<u32>,
    /// The same series by hour, oldest first; hours with no call may be left out.
    pub by_hour: Vec<HourSeries>,
}

/// One hour of a series.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct HourSeries {
    /// The start of the hour, in UTC (minutes and seconds are 00).
    pub hour: Timestamp,
    #[ts(type = "number")]
    pub count: u64,
    #[ts(type = "number")]
    pub errors: u64,
    /// The 95th percentile of the hour, as above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95_ms: Option<u32>,
}

/// `GET /api/metrics?days=<1 to 30>&kind=&name=`: the node's own measurements over a range.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Metrics {
    /// The range: the start of its first hour and the end of its last, in UTC.
    pub from: Timestamp,
    pub until: Timestamp,
    pub generated_at: Timestamp,
    /// Ordered by kind, then name, then status.
    pub series: Vec<Series>,
}

/// Whether `days` is a range a metrics read may cover.
pub fn valid_metrics_days(days: u32) -> bool {
    (1..=MAX_METRICS_DAYS).contains(&days)
}

/// The index of the bucket a duration of `millis` falls in.
pub fn bucket_of(millis: u64) -> usize {
    BUCKET_BOUNDS_MS
        .iter()
        .position(|bound| millis <= u64::from(*bound))
        .unwrap_or(BUCKET_BOUNDS_MS.len())
}

/// The `percent`th percentile (1 to 100) of the durations in `buckets`, by nearest rank, as the upper
/// bound of its bucket in milliseconds. `None` without durations, or when it lies in the last bucket.
pub fn percentile_ms(buckets: &[u64], percent: u64) -> Option<u32> {
    let total: u64 = buckets.iter().sum();
    if total == 0 || !(1..=100).contains(&percent) {
        return None;
    }
    let rank = (total * percent).div_ceil(100);
    let mut seen = 0;
    for (index, count) in buckets.iter().enumerate() {
        seen += count;
        if seen >= rank {
            return BUCKET_BOUNDS_MS.get(index).copied();
        }
    }
    None
}

impl Series {
    /// Whether the numbers of the series agree with each other: `BUCKETS` buckets that add up to `count`,
    /// no more errors than calls, and the percentiles those of the buckets (so a reader can trust either).
    pub fn is_consistent(&self) -> bool {
        self.buckets.len() == BUCKETS
            && self.buckets.iter().sum::<u64>() == self.count
            && self.errors <= self.count
            && self.p50_ms == percentile_ms(&self.buckets, 50)
            && self.p95_ms == percentile_ms(&self.buckets, 95)
            && self.p99_ms == percentile_ms(&self.buckets, 99)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_falls_in_the_first_bucket_whose_bound_it_does_not_pass() {
        assert_eq!(bucket_of(0), 0);
        assert_eq!(bucket_of(1), 0);
        assert_eq!(bucket_of(2), 1);
        assert_eq!(bucket_of(3), 2);
        assert_eq!(bucket_of(100), 6);
        assert_eq!(bucket_of(101), 7);
        assert_eq!(bucket_of(30_000), 13);
        assert_eq!(bucket_of(30_001), 14, "slower than the last bound");
        assert_eq!(BUCKETS, 15);
        assert!(BUCKET_BOUNDS_MS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn buckets(spec: &[(usize, u64)]) -> Vec<u64> {
        let mut all = vec![0; BUCKETS];
        for (index, count) in spec {
            all[*index] = *count;
        }
        all
    }

    #[test]
    fn a_percentile_is_the_upper_bound_of_the_bucket_of_its_nearest_rank() {
        // 10 durations: 5 in 1 ms, 4 in 25 ms, 1 in 1000 ms.
        let spec = buckets(&[(0, 5), (4, 4), (9, 1)]);
        assert_eq!(
            percentile_ms(&spec, 50),
            Some(1),
            "rank 5 is the fifth, in the first bucket"
        );
        assert_eq!(percentile_ms(&spec, 51), Some(25), "rank 6");
        assert_eq!(percentile_ms(&spec, 90), Some(25), "rank 9");
        assert_eq!(percentile_ms(&spec, 91), Some(1000), "rank 10");
        assert_eq!(percentile_ms(&spec, 100), Some(1000));
        assert_eq!(
            percentile_ms(&buckets(&[(14, 3)]), 50),
            None,
            "slower than the last bound has no bound"
        );
        assert_eq!(percentile_ms(&buckets(&[]), 50), None, "nothing to give");
        assert_eq!(percentile_ms(&spec, 0), None);
        assert_eq!(percentile_ms(&spec, 101), None);
        assert_eq!(
            percentile_ms(&buckets(&[(3, 1)]), 99),
            Some(10),
            "one duration is every percentile"
        );
    }

    #[test]
    fn ranges_are_one_to_thirty_days() {
        assert!(valid_metrics_days(1) && valid_metrics_days(30));
        assert!(!valid_metrics_days(0) && !valid_metrics_days(31));
    }
}
