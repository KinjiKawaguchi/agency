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
/// The longest a series' name may be, in characters.
pub const MAX_NAME_CHARS: usize = 120;
/// The methods a name of a route may begin with; any other method is `OTHER`, because a requester picks the
/// method and a name made from it would otherwise be unbounded.
pub const METHODS: [&str; 8] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "OTHER",
];
/// The names of a route that no template names: a request that matched no route (every 404 and scanner is
/// one series), a series the node had no room for, and a name that was not one a series may have.
pub const UNMATCHED: &str = "unmatched";
pub const OVERFLOW: &str = "overflow";
pub const INVALID: &str = "invalid";
/// The clients that report what they observed.
pub const CLIENTS: [&str; 2] = ["web", "ios"];
/// The performance budget of an in-node read, in milliseconds at the 95th percentile, in the reference
/// environment the project names. A percentile is the upper bound of its bucket and 100 is one of the
/// bounds, so `p95Ms <= 100` is the test.
pub const IN_NODE_READ_P95_BUDGET_MS: u32 = 100;
/// The groups a route belongs to, which a screen has words for (a `serve` or `client` series).
pub const ROUTE_GROUPS: [&str; 11] = [
    "desk",
    "teams",
    "usage",
    "evaluation",
    "outcomes",
    "devices",
    "push",
    "federation",
    "metrics",
    "unmatched",
    "other",
];
/// The groups of a `call` series.
pub const CALL_GROUPS: [&str; 5] = ["runtime", "peer", "push", "github", "other"];

/// The group of a series of `kind` named `name`, which a screen draws against: for a route (`serve`,
/// `client`) by its template, and for a call by its first word. The node says it, so that a screen holds no
/// table of the node's routes.
pub fn group_of(kind: SeriesKind, name: &str) -> &'static str {
    match kind {
        SeriesKind::Call => match name.split(' ').next().unwrap_or("") {
            "runtime" => "runtime",
            "peer" => "peer",
            "apns" | "webpush" => "push",
            "github" => "github",
            _ => "other",
        },
        SeriesKind::Serve | SeriesKind::Client => {
            if name == UNMATCHED {
                return "unmatched";
            }
            let template = name.split_once(' ').map_or("", |(_, template)| template);
            route_group(template)
        }
    }
}

fn route_group(template: &str) -> &'static str {
    let starts = |prefix: &str| template.starts_with(prefix);
    if starts("/fed/v1/") {
        "federation"
    } else if template == "/metrics" || starts("/api/metrics") || template == "/api/telemetry" {
        "metrics"
    } else if starts("/api/push") {
        "push"
    } else if starts("/api/devices") {
        "devices"
    } else if starts("/api/orgs/{org}/outcomes") {
        "outcomes"
    } else if starts("/api/orgs/{org}/evaluation") || starts("/api/orgs/{org}/effort") {
        "evaluation"
    } else if starts("/api/orgs/{org}/teams/{team}/usage")
        || starts("/api/orgs/{org}/teams/{team}/budget")
    {
        "usage"
    } else if template == "/api/orgs/{org}"
        || starts("/api/orgs/{org}/history")
        || starts("/api/orgs/{org}/teams/")
    {
        "teams"
    } else if ["/api/me", "/api/session", "/api/decisions", "/api/about"].contains(&template) {
        "desk"
    } else {
        "other"
    }
}

/// Whether a series is a read the node answers from its own state, which the budget of
/// `IN_NODE_READ_P95_BUDGET_MS` applies to: a `serve` series of a GET of the desk, the devices, the push
/// settings, the outcomes the owner entered, an org, a team's view (kept in memory and refreshed in the
/// background), and, between nodes, an org's about and versions and a team's view. What calls the team's
/// runtime, GitHub, or a peer (usage, evaluation, activity) is not one.
pub fn is_in_node_read(kind: SeriesKind, name: &str) -> bool {
    if kind != SeriesKind::Serve {
        return false;
    }
    let Some(("GET", template)) = name.split_once(' ') else {
        return false;
    };
    matches!(
        group_of(kind, name),
        "desk" | "devices" | "push" | "outcomes"
    ) || [
        "/api/orgs/{org}",
        "/api/orgs/{org}/teams/{team}/view",
        "/fed/v1/orgs/{org}/about",
        "/fed/v1/orgs/{org}/versions",
        "/fed/v1/teams/{org}/{team}/view",
    ]
    .contains(&template)
}

/// Whether an objective the operator set for a group is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ObjectiveState {
    /// The 95th percentile of the group is within the objective: the upper bound of its bucket is at most
    /// the objective.
    Met,
    /// It is not: every duration in its bucket is above the objective (the bucket's lower bound is at or
    /// above it), which includes a 95th percentile slower than the last bound (30 000 ms).
    NotMet,
    /// The objective falls inside the bucket of the 95th percentile, so the measurement cannot say: a
    /// bucket is the most that is known of a duration. A screen shows it as that, not as met or not met.
    Undetermined,
    /// There is nothing to compare: the group had no calls in the range.
    Unknown,
}

/// An objective the operator set for the node (`metrics.objectives` in `config.json`) and how the
/// measurements stand against it over the range asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ObjectiveResult {
    /// The group of routes the objective is for (`group` of a `serve` series).
    pub group: String,
    /// The most the 95th percentile of the group may be, in milliseconds: what the operator set.
    pub p95_ms: u32,
    pub state: ObjectiveState,
    /// The 95th percentile it was compared to, as the upper bound of its bucket; left out when there is
    /// none (no calls, or slower than 30 000 ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_p95_ms: Option<u32>,
}

/// How the `serve` series of `group` stand against an objective of `p95_ms`: the buckets of all of the
/// group's series (every route and status of it) are added up and the bucket of their 95th percentile is
/// compared: `Met` when its upper bound is at most the objective, `NotMet` when its lower bound (the bound
/// before it, 0 for the first) is at or above it, and `Undetermined` when the objective falls inside it.
pub fn judge(group: &str, p95_ms: u32, series: &[Series]) -> ObjectiveResult {
    let mut buckets = [0u64; BUCKETS];
    let mut is_too_many = false;
    for one in series
        .iter()
        .filter(|one| one.kind == SeriesKind::Serve && one.group == group)
    {
        for (total, count) in buckets.iter_mut().zip(&one.buckets) {
            match total.checked_add(*count) {
                Some(sum) => *total = sum,
                None => is_too_many = true,
            }
        }
    }
    // More durations than a count holds are not ones a node recorded: nothing is said of them.
    let bucket = match is_too_many {
        true => None,
        false => percentile_bucket(&buckets, 95),
    };
    let measured = bucket.and_then(|index| BUCKET_BOUNDS_MS.get(index).copied());
    let state = match bucket {
        None => ObjectiveState::Unknown,
        Some(index) => {
            let lower = index
                .checked_sub(1)
                .map_or(0, |below| BUCKET_BOUNDS_MS[below]);
            match measured {
                Some(upper) if upper <= p95_ms => ObjectiveState::Met,
                _ if lower >= p95_ms => ObjectiveState::NotMet,
                _ => ObjectiveState::Undetermined,
            }
        }
    };
    ObjectiveResult {
        group: group.to_owned(),
        p95_ms,
        state,
        measured_p95_ms: measured,
    }
}

/// What the answer says of the budget, so that a screen does not decide it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Budget {
    /// The most the 95th percentile of an in-node read may be, in milliseconds (`IN_NODE_READ_P95_BUDGET_MS`).
    pub in_node_read_p95_ms: u32,
}

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
    /// The group of the series (`group_of`): for a screen to draw against, not for it to work out.
    pub group: String,
    /// Whether the budget applies to the series (`is_in_node_read`).
    pub is_in_node_read: bool,
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
    /// How many durations of the hour fell in each bucket, as `Series.buckets` (`BUCKETS` of them, adding up
    /// to the hour's `count`), so that a reader can add the hours of several series and read the percentile of
    /// the whole. Only in an answer asked for it (`hourBuckets=1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "Array<number>", optional)]
    pub buckets: Option<Vec<u64>>,
}

impl HourSeries {
    /// Whether the hour agrees with itself: when it has buckets they are `BUCKETS` that add up to `count`,
    /// and its percentile is that of the buckets.
    pub fn is_consistent(&self) -> bool {
        self.buckets.as_ref().is_none_or(|buckets| {
            buckets.len() == BUCKETS
                && bucket_total(buckets) == Some(self.count)
                && self.p95_ms == percentile_ms(buckets, 95)
        })
    }
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
    /// The budget an in-node read is held to.
    pub budget: Budget,
    /// How the groups the operator set objectives for stand against them. Left out when there are none, and
    /// in an answer narrowed to a kind or a name (which would not be the whole of a group).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<ObjectiveResult>>", optional)]
    pub objectives: Vec<ObjectiveResult>,
    /// Ordered by kind, then name, then status.
    pub series: Vec<Series>,
}

/// Whether `name` is one a series of `kind` may have: not empty, at most `MAX_NAME_CHARS`, no control
/// character, `?`, or `#` (so never a query or a fragment); for a route (`serve`, `client`) a method of
/// `METHODS` and a template, or one of the fixed names `UNMATCHED`, `OVERFLOW`, and `INVALID`; for a call,
/// a fixed phrase, or `OVERFLOW` or `INVALID`.
pub fn valid_name(kind: SeriesKind, name: &str) -> bool {
    let is_plain = !name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.chars().any(|c| c.is_control() || c == '?' || c == '#');
    if !is_plain {
        return false;
    }
    if [OVERFLOW, INVALID].contains(&name) {
        return true;
    }
    match kind {
        SeriesKind::Call => true,
        SeriesKind::Serve | SeriesKind::Client => {
            name == UNMATCHED
                || name.split_once(' ').is_some_and(|(method, template)| {
                    METHODS.contains(&method) && !template.is_empty()
                })
        }
    }
}

/// Whether `status` is one a series of `kind` may have.
pub fn valid_status(kind: SeriesKind, status: &str) -> bool {
    let classes = ["2xx", "3xx", "4xx", "5xx"];
    match kind {
        SeriesKind::Serve => classes.contains(&status),
        SeriesKind::Call => {
            classes.contains(&status) || ["ok", "error", "timeout"].contains(&status)
        }
        SeriesKind::Client => classes.contains(&status) || status == "network",
    }
}

/// Whether a call of `kind` with `status` is an error: a 5xx; for a call, also `error` and `timeout`; for
/// what a client observed, also `network`. A series has one status, so all of its calls are or none are.
pub fn is_error_status(kind: SeriesKind, status: &str) -> bool {
    status == "5xx"
        || (kind == SeriesKind::Call && ["error", "timeout"].contains(&status))
        || (kind == SeriesKind::Client && status == "network")
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
    percentile_bucket(buckets, percent).and_then(|index| BUCKET_BOUNDS_MS.get(index).copied())
}

/// How many durations the buckets hold, or `None` when they hold more than a `u64` counts: a count that
/// large is not one a node recorded, so what carries it is read as inconsistent and not as a wrapped sum.
pub fn bucket_total(buckets: &[u64]) -> Option<u64> {
    buckets
        .iter()
        .try_fold(0u64, |total, count| total.checked_add(*count))
}

/// The index of the bucket the `percent`th percentile of the durations in `buckets` lies in, by nearest rank.
/// `None` without durations or for a percent outside 1 to 100.
pub fn percentile_bucket(buckets: &[u64], percent: u64) -> Option<usize> {
    let total = bucket_total(buckets)?;
    if total == 0 || !(1..=100).contains(&percent) {
        return None;
    }
    let rank = u64::try_from((u128::from(total) * u128::from(percent)).div_ceil(100)).ok()?;
    let mut seen = 0;
    for (index, count) in buckets.iter().enumerate() {
        seen += count;
        if seen >= rank {
            return Some(index);
        }
    }
    None
}

impl Series {
    /// Whether the series is what the protocol allows: a name, group, status, peer, and client that a series of its
    /// kind may have (the answer holds nothing a requester could choose freely), `BUCKETS` buckets that add up
    /// to `count`, errors that are all of the calls or none as the status says, and the percentiles those of
    /// the buckets (so a reader can trust either).
    /// Leaves the buckets of the hours out, as the answer does unless it is asked for them.
    pub fn without_hour_buckets(mut self) -> Self {
        for hour in &mut self.by_hour {
            hour.buckets = None;
        }
        self
    }

    pub fn is_consistent(&self) -> bool {
        let errors_agree = if is_error_status(self.kind, &self.status) {
            self.errors == self.count
        } else {
            self.errors == 0
        };
        valid_name(self.kind, &self.name)
            && self.group == group_of(self.kind, &self.name)
            && self.is_in_node_read == is_in_node_read(self.kind, &self.name)
            && valid_status(self.kind, &self.status)
            && (self.peer.is_none() || self.kind == SeriesKind::Call)
            && self
                .client
                .as_deref()
                .is_none_or(|client| CLIENTS.contains(&client))
            && self.client.is_some() == (self.kind == SeriesKind::Client)
            && errors_agree
            && self.by_hour.iter().all(HourSeries::is_consistent)
            && self.buckets.len() == BUCKETS
            && bucket_total(&self.buckets) == Some(self.count)
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
