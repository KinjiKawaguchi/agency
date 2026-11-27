//! Since when a team has been where it is (docs/protocol.md, "Decision envelopes"). The node a team ran on
//! before received every envelope for it, and the node it runs on now keeps only what it applied itself, so an
//! envelope made before the team came to its node was made for another node, and is not taken here.
use crate::node::Node;
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::envelope::MAX_AGE_SECONDS;
use agency_proto::org::SignedOrg;
use agency_proto::Id;

/// When the team was placed on the node `current` names for it: the time of the oldest version, going back from
/// `current`, of the unbroken run that names that same node for it. `None` when the run reaches back further
/// than an envelope can be old (every envelope still fresh was made after it), or when a version of the run is
/// not kept here (nothing can be said, so nothing is refused for it).
pub(crate) fn placed_at(
    node: &Node,
    current: &SignedOrg,
    team: &Id,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    let org = &current.body.org;
    let place = current.body.team(team)?.node.clone();
    let mut earliest = current.clone();
    loop {
        let at = clock::parse(&earliest.body.at)?;
        if (now - at).whole_seconds() > MAX_AGE_SECONDS {
            return None;
        }
        if earliest.body.version <= 1 {
            return Some(at);
        }
        let before = node
            .store()
            .load_org_version(org, earliest.body.version - 1)?;
        let is_same_place = before.body.team(team).is_some_and(|one| one.node == place);
        if !is_same_place {
            return Some(at);
        }
        earliest = before;
    }
}
