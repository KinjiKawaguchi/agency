//! The root's check before it signs an org version that uses a field older nodes refuse (docs/compatibility.md,
//! "Signed data"). A node refuses a definition with a field it does not know, so a version that carries a
//! new field must not be written until every node that reads it lists the capability for it. The check reads
//! only what this node has kept of its peers (`peer_about`): signing never waits for the network.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::peer_about::{peer_origins, peers_of, PeerAbout};
use agency_proto::about::Capability;
use agency_proto::clock::{self, Duration, OffsetDateTime};
use agency_proto::OrgDefinition;
use std::collections::BTreeSet;

/// How old a peer's answer may be and still count. An older one counts as no answer: the peer may have been
/// replaced since.
pub const MAX_AGE: Duration = Duration::days(3);

/// Why `origin` may not be relied on for `needed`, or `None` when it lists them all in a recent answer.
fn gap(
    origin: &str,
    kept: Option<&PeerAbout>,
    needed: &BTreeSet<Capability>,
    now: OffsetDateTime,
) -> Option<String> {
    let Some(kept) = kept else {
        return Some(format!("{origin} has not been read yet"));
    };
    let Some(about) = &kept.about else {
        let why = kept.error.as_deref().unwrap_or("no answer");
        return Some(format!("{origin} has not answered ({why})"));
    };
    let age = kept
        .read_at
        .as_deref()
        .and_then(clock::parse)
        .map(|at| now - at);
    if age.is_none_or(|age| age > MAX_AGE) {
        let why = kept.error.as_deref().unwrap_or("it was not read again");
        return Some(format!(
            "{origin} was last read more than {} days ago ({why})",
            MAX_AGE.whole_days()
        ));
    }
    let missing: Vec<&str> = needed
        .iter()
        .filter(|one| !about.lists(**one))
        .map(|one| one.name())
        .collect();
    (!missing.is_empty()).then(|| {
        format!(
            "{origin} does not list {} (it runs release {})",
            missing.join(", "),
            about.node
        )
    })
}

/// Refuses a version, before it is signed, that uses a capability the current one does not use yet while a
/// node of the org (the nodes of the new version's teams that do not run here) does not list it. A field the
/// current version already carries is not checked again: a node that cannot read it refuses the current
/// version already, and refusing every later one would not help it. Taking such a node's teams out of the
/// org leaves it out of the check.
pub(crate) fn check_before_signing(
    node: &Node,
    current: &OrgDefinition,
    next: &OrgDefinition,
) -> Result<()> {
    let already = Capability::used_by(current);
    let needed: BTreeSet<Capability> = Capability::used_by(next)
        .into_iter()
        .filter(|one| !already.contains(one))
        .collect();
    if needed.is_empty() {
        return Ok(());
    }
    let kept = peers_of(node, &next.org);
    let now = node.now();
    let behind: Vec<String> = peer_origins(node, next)
        .iter()
        .filter_map(|origin| gap(origin, kept.get(origin), &needed, now))
        .collect();
    if behind.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = needed.iter().map(|one| one.name()).collect();
    Err(Error::new(
        ErrorKind::Conflict,
        format!(
            "cannot sign a version that uses {}, which older nodes refuse: {}. Update those nodes, then run `agency sync` to read them again",
            names.join(", "),
            behind.join("; ")
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::about::About;

    const NOW: &str = "2026-11-24T12:00:00Z";
    const ORIGIN: &str = "https://b.example";

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    fn needed() -> BTreeSet<Capability> {
        BTreeSet::from([Capability::OrgPlan, Capability::OrgKpis])
    }

    fn read(about: About, read_at: &str) -> PeerAbout {
        PeerAbout {
            about: Some(about),
            read_at: Some(read_at.into()),
            tried_at: read_at.into(),
            ..PeerAbout::default()
        }
    }

    fn without(name: &str) -> About {
        let mut about = About::of_release("0.3.0");
        about.capabilities.retain(|one| one != name);
        about
    }

    fn gap_of(kept: Option<&PeerAbout>) -> Option<String> {
        gap(ORIGIN, kept, &needed(), at(NOW))
    }

    #[test]
    fn a_node_that_lists_everything_in_a_recent_answer_is_not_behind() {
        let kept = read(About::of_release("0.3.0"), "2026-11-24T11:00:00Z");
        assert_eq!(gap_of(Some(&kept)), None);
    }

    #[test]
    fn a_node_is_behind_for_exactly_the_capabilities_it_lacks_and_is_named_with_its_release() {
        let kept = read(without("orgKpis"), NOW);
        let said = gap_of(Some(&kept)).unwrap();
        assert!(
            said.contains(ORIGIN) && said.contains("orgKpis") && said.contains("0.3.0"),
            "{said}"
        );
        assert!(!said.contains("orgPlan"), "it lists the plan: {said}");
        let lacks_what_is_not_needed = read(without("webPush"), NOW);
        assert_eq!(gap_of(Some(&lacks_what_is_not_needed)), None);
    }

    #[test]
    fn a_node_never_read_or_that_never_answered_is_behind() {
        assert!(gap_of(None).unwrap().contains("has not been read"));
        let failed = PeerAbout {
            error: Some("unreachable".into()),
            tried_at: NOW.into(),
            ..PeerAbout::default()
        };
        assert!(gap_of(Some(&failed)).unwrap().contains("unreachable"));
    }

    #[test]
    fn an_answer_older_than_three_days_counts_as_no_answer() {
        let recent = read(About::of_release("0.3.0"), "2026-11-21T12:00:00Z");
        assert_eq!(
            gap_of(Some(&recent)),
            None,
            "exactly three days is still recent"
        );
        let old = read(About::of_release("0.3.0"), "2026-11-21T11:59:59Z");
        assert!(gap_of(Some(&old)).unwrap().contains("more than 3 days"));
        let failing = PeerAbout {
            error: Some("unreachable".into()),
            ..old
        };
        assert!(
            gap_of(Some(&failing)).unwrap().contains("unreachable"),
            "and says why it is not newer"
        );
        let timeless = PeerAbout {
            read_at: None,
            ..read(About::of_release("0.3.0"), NOW)
        };
        assert!(
            gap_of(Some(&timeless)).is_some(),
            "an answer with no time is not recent"
        );
    }
}
