//! The day each epic a team's plan aims at a KPI shipped, remembered by this node. A team's view keeps a
//! finished epic only for a limited time, while a reading looks 90 days past the ship day, so the node
//! writes the time it first sees an epic finished into its ledger and never changes it (docs/protocol.md,
//! "Readings"). Only epics that an aim names are remembered.
use crate::error::Result;
use crate::node::Node;
use crate::ports::ShipRecord;
use crate::views::runtime_view;
use agency_proto::{Id, OrgDefinition, TeamView, Timestamp};
use std::collections::BTreeMap;

/// The time each epic was first seen finished, by team and item reference.
pub(crate) type Shipped = BTreeMap<(Id, String), Timestamp>;

/// What the ledger holds for the org: the first record of each epic.
pub(crate) fn remembered(node: &Node, org: &Id) -> Shipped {
    let mut first = Shipped::new();
    for record in node.store().ships(org) {
        first
            .entry((record.team, record.epic))
            .or_insert(record.shipped_at);
    }
    first
}

/// The views, as this node has them, of the teams of `definition` whose plan has aims: read from the
/// runtime for a team that runs here (which answers from memory), from the replica for one on another node.
/// A team whose view is not there has `None`.
pub(crate) async fn aimed_views(
    node: &Node,
    definition: &OrgDefinition,
) -> BTreeMap<Id, Option<TeamView>> {
    let mut views = BTreeMap::new();
    let aimed = definition.teams.iter().filter(|place| {
        place
            .plan
            .as_ref()
            .is_some_and(|plan| !plan.aims.is_empty())
    });
    for place in aimed {
        let view = match node.hosted(&definition.org, &place.id) {
            Some(hosted) => runtime_view(node, &hosted).await.ok(),
            None => node
                .store()
                .load_replica(&definition.org, &place.id)
                .and_then(|replica| replica.view),
        };
        views.insert(place.id.clone(), view);
    }
    views
}

/// The time an epic's last child was finished, when `view` lists the epic and says so.
fn finished_at<'a>(view: &'a TeamView, epic: &str) -> Option<&'a Timestamp> {
    view.epics
        .iter()
        .find(|one| one.reference == epic)?
        .done_at
        .as_ref()
}

/// Writes down the aimed epics that `views` show finished and the ledger does not hold yet, and gives every
/// ship time the node knows. An epic already in the ledger keeps its first time.
pub(crate) fn remember(
    node: &Node,
    definition: &OrgDefinition,
    views: &BTreeMap<Id, Option<TeamView>>,
) -> Result<Shipped> {
    let mut known = remembered(node, &definition.org);
    for place in &definition.teams {
        let (Some(plan), Some(Some(view))) = (&place.plan, views.get(&place.id)) else {
            continue;
        };
        for aim in &plan.aims {
            let key = (place.id.clone(), aim.epic.clone());
            let Some(at) = finished_at(view, &aim.epic).filter(|_| !known.contains_key(&key))
            else {
                continue;
            };
            let record = ShipRecord {
                team: place.id.clone(),
                epic: aim.epic.clone(),
                shipped_at: at.clone(),
            };
            node.store().append_ship(&definition.org, &record)?;
            known.insert(key, record.shipped_at);
        }
    }
    Ok(known)
}

/// Remembers the ship days of every org the node's account belongs to. It runs in the node's own loop, so
/// that an epic that shipped while nobody read is not lost when its team's view lets it go.
pub async fn record_all(node: &Node) -> Result<()> {
    let account = node.account();
    for doc in node
        .store()
        .orgs()
        .into_iter()
        .filter(|doc| doc.body.member(&account).is_some())
    {
        let views = aimed_views(node, &doc.body).await;
        remember(node, &doc.body, &views)?;
    }
    Ok(())
}
