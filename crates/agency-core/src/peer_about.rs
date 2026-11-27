//! What this node has read of the other nodes of an org: their release, contracts and capabilities
//! (docs/compatibility.md, "Discovery"). It is read by a signed read of each peer's `about`, kept for the org
//! and the peer's origin, read again daily, and at once (but not more than every ten minutes) after a call to
//! the peer answers 404. A failed read keeps the last good answer and says why it failed; the root's check
//! (see `orgs::amend`) treats an answer that is not recent as no answer.
use crate::error::Result;
use crate::node::Node;
use crate::read_auth::sign_for_peer;
use crate::routes;
use agency_proto::about::About;
use agency_proto::clock::{self, Duration, OffsetDateTime};
use agency_proto::{Id, OrgDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How long an answer is left before it is read again.
pub const REFRESH_AFTER: Duration = Duration::hours(24);
/// How long after a failed read, or after a 404 from the peer, before it is read again.
pub const RETRY_AFTER: Duration = Duration::minutes(10);

/// One peer node, as this node last read it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerAbout {
    /// What the peer answered when it was last read. Kept when a later read fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<About>,
    /// When `about` was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<String>,
    /// When a read was last tried.
    #[serde(default)]
    pub tried_at: String,
    /// Why the last read failed. Absent after a good read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// A call to the peer answered 404 since it was read: read it again soon.
    #[serde(default)]
    pub should_read_again: bool,
}

/// The origins of the org's other nodes: the nodes of its teams that do not run here, as `publish` sends
/// each version to.
pub(crate) fn peer_origins(node: &Node, definition: &OrgDefinition) -> BTreeSet<String> {
    let me = node.url();
    let org = &definition.org;
    let elsewhere = definition
        .teams
        .iter()
        .filter(|team| !node.runs_here(org, &team.id));
    elsewhere
        .map(|team| team.node.clone())
        .filter(|url| url != &me)
        .collect()
}

/// Whether the peer is to be read now: never read, a failed read or a 404 since and ten minutes passed, or a
/// good read a day old.
pub(crate) fn is_due(kept: Option<&PeerAbout>, now: OffsetDateTime) -> bool {
    let Some(kept) = kept else { return true };
    let is_older_than = |at: Option<&str>, limit: Duration| {
        at.and_then(clock::parse).is_none_or(|at| now - at >= limit)
    };
    if kept.error.is_some() || kept.should_read_again {
        return is_older_than(Some(&kept.tried_at), RETRY_AFTER);
    }
    is_older_than(kept.read_at.as_deref(), REFRESH_AFTER)
}

async fn read_one(node: &Node, org: &Id, origin: &str, previous: Option<&PeerAbout>) -> PeerAbout {
    let path = routes::about(org);
    let auth = sign_for_peer(node, org, origin, &path);
    let now = node.now_text();
    let answer = match node.peers().get_about(origin, &path, &auth).await {
        Ok(about) => about.check().map(|()| about),
        Err(error) => Err(error.to_string()),
    };
    match answer {
        Ok(about) => PeerAbout {
            about: Some(about),
            read_at: Some(now.clone()),
            tried_at: now,
            error: None,
            should_read_again: false,
        },
        Err(error) => PeerAbout {
            tried_at: now,
            error: Some(error),
            should_read_again: false,
            ..previous.cloned().unwrap_or_default()
        },
    }
}

/// Reads the peers of `definition`'s org that are due, and forgets those that are no longer nodes of the
/// org. A peer that does not answer does not fail this: its entry says why.
pub async fn refresh(node: &Node, definition: &OrgDefinition) -> Result<()> {
    let org = &definition.org;
    let origins = peer_origins(node, definition);
    let before = node.store().peer_abouts(org);
    let mut kept = before.clone();
    kept.retain(|origin, _| origins.contains(origin));
    for origin in &origins {
        if is_due(kept.get(origin), node.now()) {
            let next = read_one(node, org, origin, kept.get(origin)).await;
            if next.error.is_none() {
                // It answered: the replicas of its teams that waited for it are read at the next pass.
                if let Err(error) = crate::views::note_contact(node, origin) {
                    node.journal().warn(
                        "replicas",
                        &format!("could not end the wait for a node that answered: {error}"),
                    );
                }
            }
            kept.insert(origin.clone(), next);
        }
    }
    if kept != before {
        node.store().save_peer_abouts(org, &kept)?;
    }
    Ok(())
}

/// Reads one node now and keeps what it said, for a node that an edit is about to bring into the org (a first
/// team on it, or a team moved to it): the root cannot read a node it does not know until a team names it, but a
/// node that joined the org holds its definition and so can answer. The next `refresh` forgets the entry if no
/// team names the node by then.
pub async fn read_origin(node: &Node, org: &Id, origin: &str) -> Result<()> {
    let mut kept = node.store().peer_abouts(org);
    let next = read_one(node, org, origin, kept.get(origin)).await;
    kept.insert(origin.to_owned(), next);
    node.store().save_peer_abouts(org, &kept)
}

/// A call to `origin` answered 404: its entry is read again once ten minutes have passed since the last
/// try, since the peer may not be the release it was read as. A peer never read is read anyway.
pub fn note_not_found(node: &Node, org: &Id, origin: &str) -> Result<()> {
    let mut kept = node.store().peer_abouts(org);
    match kept.get_mut(origin) {
        Some(entry) if !entry.should_read_again => entry.should_read_again = true,
        _ => return Ok(()),
    }
    node.store().save_peer_abouts(org, &kept)
}

/// What this node has read of the other nodes of `org`, by origin.
pub fn peers_of(node: &Node, org: &Id) -> BTreeMap<String, PeerAbout> {
    node.store().peer_abouts(org)
}

/// Whether `origin`, a node of `org`, listed `capability` in the `about` this node last kept of it. A node never
/// read lists nothing.
pub(crate) fn lists(
    node: &Node,
    org: &Id,
    origin: &str,
    capability: agency_proto::about::Capability,
) -> bool {
    node.store()
        .peer_abouts(org)
        .get(origin)
        .and_then(|kept| kept.about.as_ref())
        .is_some_and(|about| about.lists(capability))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    const NOW: &str = "2026-11-24T12:00:00Z";

    fn read(read_at: &str) -> PeerAbout {
        PeerAbout {
            about: Some(About::of_release("0.3.0")),
            read_at: Some(read_at.into()),
            tried_at: read_at.into(),
            ..PeerAbout::default()
        }
    }

    #[test]
    fn a_peer_never_read_is_read_now() {
        assert!(is_due(None, at(NOW)));
    }

    #[test]
    fn a_good_read_is_left_for_a_day() {
        assert!(!is_due(Some(&read("2026-11-23T12:00:01Z")), at(NOW)));
        assert!(
            is_due(Some(&read("2026-11-23T12:00:00Z")), at(NOW)),
            "exactly a day old is due"
        );
        assert!(is_due(Some(&read("2026-11-20T12:00:00Z")), at(NOW)));
    }

    #[test]
    fn a_failed_read_is_tried_again_after_ten_minutes_not_before() {
        let failed = |tried_at: &str| PeerAbout {
            tried_at: tried_at.into(),
            error: Some("unreachable".into()),
            ..read("2026-11-20T12:00:00Z")
        };
        assert!(!is_due(Some(&failed("2026-11-24T11:50:01Z")), at(NOW)));
        assert!(is_due(Some(&failed("2026-11-24T11:50:00Z")), at(NOW)));
    }

    #[test]
    fn a_404_asks_for_a_read_after_ten_minutes_since_the_last_try() {
        let asked = |tried_at: &str| PeerAbout {
            tried_at: tried_at.into(),
            should_read_again: true,
            ..read(tried_at)
        };
        assert!(
            !is_due(Some(&asked("2026-11-24T11:55:00Z")), at(NOW)),
            "read five minutes ago"
        );
        assert!(is_due(Some(&asked("2026-11-24T11:50:00Z")), at(NOW)));
    }

    #[test]
    fn an_entry_with_no_readable_time_is_read() {
        let broken = PeerAbout {
            read_at: Some("not a time".into()),
            ..read(NOW)
        };
        assert!(is_due(Some(&broken), at(NOW)));
    }
}
