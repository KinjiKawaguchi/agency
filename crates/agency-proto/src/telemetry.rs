//! What a client reports of its own calls to its home node (`POST /api/telemetry`, docs/protocol.md,
//! "Metrics"): how long the person waited for an answer, by route template and status. It holds no path
//! with an id, no query, no body, and nothing about the person; the node keeps it as `client` series.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The most samples a report may hold.
pub const MOST_SAMPLES: usize = 200;
/// The most bytes a report may be.
pub const MOST_REPORT_BYTES: usize = 16 * 1024;
/// The longest a sample counts as, in milliseconds: one more than the last bound of the buckets, so a longer
/// wait is in the last bucket whatever number the client measured.
pub const MOST_SAMPLE_MS: u64 = 30_001;

/// A batch of what a client observed. `client` is `web` or `ios`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TelemetryReport {
    pub client: String,
    pub samples: Vec<TelemetrySample>,
}

/// One call a client made: the route as the node names it (`GET /api/orgs/{org}/teams/{team}/view`, from
/// `testdata/route-templates.json`), the class of its status or `network` when there was no answer, and the
/// milliseconds from the start of the request until the body was read (or the failure came).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TelemetrySample {
    pub route: String,
    pub status: String,
    #[ts(type = "number")]
    pub ms: u64,
}

/// What the node did with a report: the samples it kept, and the ones it dropped because their route is not
/// one it knows or their status is not one a series may have.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TelemetryAnswer {
    #[ts(type = "number")]
    pub accepted: u64,
    #[ts(type = "number")]
    pub dropped: u64,
}
