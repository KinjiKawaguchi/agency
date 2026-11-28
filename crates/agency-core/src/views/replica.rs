use agency_proto::clock::{self, Duration, OffsetDateTime};
use agency_proto::view::AnswersFor;
use agency_proto::TeamView;
use serde::{Deserialize, Serialize};

/// A replica of a team on another node. When a read fails, the previous view stays and only the error is recorded.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replica {
    pub fetched_at: Option<String>,
    pub tried_at: String,
    pub error: Option<String>,
    pub view: Option<TeamView>,
    /// This node's account's name in the team runtime, as the team's node reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
    /// The seats and external parties this node's account answers for in the team, as the team's node
    /// reported them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answers_for: Vec<AnswersFor>,
    /// How many reads in a row have failed; 0 once one succeeds. The next read waits as the outbox does
    /// (`outbox::BACKOFF_SECONDS`), counted from `tried_at`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub failures: u32,
    /// When the first of those failed reads was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing_since: Option<String>,
    /// The tag the team's node gave the answer this replica holds (`FederatedView::tag`), sent back to a node that
    /// lists `viewUnchanged` so that it answers `304` while nothing changed. Left out when the node gave none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

fn is_zero(count: &u32) -> bool {
    *count == 0
}

impl Replica {
    /// When the team's node is read next: at once after a success, else after the backoff for the failures so far.
    pub fn next_read_at(&self) -> Option<OffsetDateTime> {
        if self.failures == 0 {
            return None;
        }
        let steps = &crate::outbox::BACKOFF_SECONDS;
        let wait = steps[(self.failures as usize - 1).min(steps.len() - 1)];
        clock::parse(&self.tried_at).map(|tried| tried + Duration::seconds(wait))
    }
}
