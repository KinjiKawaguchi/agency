//! Whether the node a team runs on has an optional feature, from what this node has kept of that node's
//! `about` (docs/compatibility.md, "Node and node" and "Discovery"). A read of a remote team's feature asks
//! this before it calls the route: a node that does not list the feature's capability answers 404 for the
//! route, which a caller must not take for "the team is not there". The answer counts as the capability check
//! of signed fields counts it (`capabilities::recent_about`): one never read, one that did not answer, and one
//! older than three days say nothing.
use crate::capabilities::recent_about;
use crate::node::Node;
use crate::peer_about::{peers_of, PeerAbout};
use agency_proto::about::Capability;
use agency_proto::clock::OffsetDateTime;
use agency_proto::Id;

/// What the kept answer of a node says about one capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Support {
    /// The node lists the capability.
    Listed,
    /// The node does not list it, and runs this release.
    Lacking { release: String },
    /// Nothing usable is kept, so the node may have it.
    Unknown,
}

/// What `kept` says about `capability` at `now`.
pub fn support(kept: Option<&PeerAbout>, capability: Capability, now: OffsetDateTime) -> Support {
    match recent_about("", kept, now) {
        Err(_) => Support::Unknown,
        Ok(about) if about.lists(capability) => Support::Listed,
        Ok(about) => Support::Lacking {
            release: about.node.clone(),
        },
    }
}

/// What this node has kept of `origin`, a node of `org`, says about `capability`.
pub fn support_of_peer(node: &Node, org: &Id, origin: &str, capability: Capability) -> Support {
    support(peers_of(node, org).get(origin), capability, node.now())
}

/// Why a feature is not read from a team's node that lacks it. `what` names the feature in the words of
/// the read ("the team's activity feed"); `release` is what the node said it runs (an answer is kept only
/// with a short text without control characters).
pub fn lacking_reason(what: &str, release: &str) -> String {
    format!(
        "the team's node runs release {release}, which does not have {what}; it needs an update"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::about::About;
    use agency_proto::clock;

    const NOW: &str = "2026-11-24T12:00:00Z";

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    fn read(about: Option<About>, read_at: &str) -> PeerAbout {
        PeerAbout {
            about,
            read_at: Some(read_at.into()),
            tried_at: read_at.into(),
            ..PeerAbout::default()
        }
    }

    fn without_activity() -> About {
        let mut about = About::of_release("0.2.0");
        about.capabilities.retain(|one| one != "activity");
        about
    }

    #[test]
    fn a_recent_answer_that_lists_the_capability_says_listed() {
        let kept = read(Some(About::of_release("0.3.0")), NOW);
        assert_eq!(
            support(Some(&kept), Capability::Activity, at(NOW)),
            Support::Listed
        );
    }

    #[test]
    fn a_recent_answer_without_it_says_which_release_lacks_it() {
        let kept = read(Some(without_activity()), NOW);
        let lacking = Support::Lacking {
            release: "0.2.0".into(),
        };
        assert_eq!(support(Some(&kept), Capability::Activity, at(NOW)), lacking);
        assert_eq!(
            support(Some(&kept), Capability::Effort, at(NOW)),
            Support::Listed,
            "only the capability asked about"
        );
    }

    #[test]
    fn an_answer_that_is_missing_or_too_old_says_nothing() {
        assert_eq!(
            support(None, Capability::Activity, at(NOW)),
            Support::Unknown
        );
        let never_answered = read(None, NOW);
        assert_eq!(
            support(Some(&never_answered), Capability::Activity, at(NOW)),
            Support::Unknown
        );
        let old = read(Some(without_activity()), "2026-11-20T12:00:00Z");
        assert_eq!(
            support(Some(&old), Capability::Activity, at(NOW)),
            Support::Unknown,
            "older than three days"
        );
        let edge = read(Some(without_activity()), "2026-11-21T12:00:00Z");
        assert_eq!(
            support(Some(&edge), Capability::Activity, at(NOW)),
            Support::Lacking {
                release: "0.2.0".into()
            },
            "exactly three days still counts"
        );
    }

    #[test]
    fn the_reason_names_the_release_and_says_to_update() {
        let reason = lacking_reason("the team's activity feed", "0.2.0");
        assert!(reason.contains("release 0.2.0"), "{reason}");
        assert!(reason.contains("needs an update"), "{reason}");
    }
}
