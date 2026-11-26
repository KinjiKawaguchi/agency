//! The notifications that are not about a request: a device that asks to be added, a decision of the
//! owner's that the team's node refused, and a recovery of an org the owner is the root of. Each is told
//! once, by its key in the notified list.
use crate::client::{decision_log, MAX_LOG_LIMIT};
use crate::devices::pending_requests;
use crate::node::Node;
use crate::notice::{Notice, NoticeKind};
use crate::orgs::recovery_takes_effect;
use crate::push::Candidate;
use agency_proto::client::DecisionState;
use agency_proto::clock;

/// The devices waiting to be added, by their device id (`d-` and the pairing code). A device that asks
/// again with the same key is the same one.
pub(crate) fn device_candidates(node: &Node, is_hidden: bool) -> Vec<Candidate> {
    let Ok(requests) = pending_requests(node) else {
        return vec![];
    };
    requests
        .into_iter()
        .map(|request| Candidate {
            key: format!("device/{}", request.device_id),
            notice: Notice {
                kind: NoticeKind::Device {
                    device: request.device_id,
                    label: (!is_hidden).then_some(request.label),
                },
                badge: 0,
            },
        })
        .collect()
}

/// The decisions in the log whose state is now `refused`, by decision id. One that was queued and is
/// refused later is new when it is first seen refused.
pub(crate) fn decision_candidates(node: &Node, is_hidden: bool) -> Vec<Candidate> {
    decision_log(node, Some(MAX_LOG_LIMIT))
        .entries
        .into_iter()
        .filter(|entry| matches!(entry.state, DecisionState::Refused { .. }))
        .map(|entry| Candidate {
            key: format!("decision/{}", entry.id),
            notice: Notice {
                kind: NoticeKind::Decision {
                    org: entry.org,
                    team: entry.team,
                    id: entry.id,
                    title: entry.title.filter(|_| !is_hidden),
                },
                badge: 0,
            },
        })
        .collect()
}

/// The recoveries that wait in orgs whose root is this node's account, by org and the start's version. A
/// recovery is told even on a node's first pass, which records the rest unsent: the owner can cancel it only
/// while it waits.
pub(crate) fn recovery_candidates(node: &Node) -> Vec<Candidate> {
    let account = node.account();
    node.store()
        .orgs()
        .into_iter()
        .filter(|doc| doc.body.root.account == account && doc.body.root.pending.is_some())
        .map(|doc| Candidate {
            key: format!("recovery/{}/{}", doc.body.org, doc.body.version),
            notice: Notice {
                kind: NoticeKind::Recovery {
                    org: doc.body.org.clone(),
                    started_at: node
                        .store()
                        .recovery_started(&doc.body.org, doc.body.version),
                    takes_effect_at: recovery_takes_effect(node, &doc).map(clock::format),
                },
                badge: 0,
            },
        })
        .collect()
}

/// The changes held for the owner's `agency root-sign`, by handle. Each is told once, like a device request.
pub(crate) fn root_change_candidates(node: &Node, is_hidden: bool) -> Vec<Candidate> {
    let Ok(held) = crate::pending_root::waiting(node) else {
        return vec![];
    };
    let count = |org: &agency_proto::Id| {
        let in_org = held.iter().filter(|one| &one.org == org).count();
        u32::try_from(in_org).unwrap_or(u32::MAX)
    };
    let counted: Vec<(u32, crate::pending_root::PendingRootChange)> = held
        .iter()
        .map(|one| (count(&one.org), one.clone()))
        .collect();
    counted
        .into_iter()
        .map(|(waiting, one)| Candidate {
            key: format!("root/{}", one.notice_id()),
            notice: Notice {
                kind: NoticeKind::RootChange {
                    what: (!is_hidden).then(|| one.what()),
                    id: one.notice_id(),
                    org: one.org,
                    waiting,
                },
                badge: 0,
            },
        })
        .collect()
}
