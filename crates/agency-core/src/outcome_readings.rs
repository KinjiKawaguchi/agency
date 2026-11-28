//! How each KPI the org's plans aim at stood around the day its epic shipped (docs/protocol.md, "Readings").
//! This gathers what the reading needs (the aims, the ship days, the owner's numbers, today) and leaves the
//! arithmetic to `readings::read`. Who may call it is decided by the Client API (owner only), not here.
use crate::client::membership;
use crate::error::{Error, Result};
use crate::node::Node;
use crate::outcomes::effective;
use crate::readings::{read, Aim, Ship, Shipment};
use crate::ships::{aimed_views, remember};
use agency_proto::clock::{self, CalendarDate, UtcOffset};
use agency_proto::observation::Observation;
use agency_proto::outcome::Aim as PlannedAim;
use agency_proto::reading::{AimReading, Readings};
use agency_proto::{Id, TeamView, Timestamp};
use std::collections::BTreeMap;

/// An aim of a team's plan with what is known of its epic.
struct Planned<'a> {
    team: Id,
    aim: PlannedAim,
    title: Option<&'a str>,
    ship: Ship,
}

/// The calendar day, in the node's time zone, of a ledger time.
fn day_of(offset: UtcOffset, at: &Timestamp) -> Option<CalendarDate> {
    Some(clock::parse(at.as_str())?.to_offset(offset).date())
}

/// What is known of when an epic shipped: the ledger's day when it has one; otherwise the team's view tells
/// that the epic is not finished, or that it is finished with no known time, or does not list it at all (a
/// reference that names nothing there, a view not read yet, or an epic the team no longer keeps): nothing
/// is known then, and nothing is guessed.
fn ship_of(
    offset: UtcOffset,
    view: Option<&TeamView>,
    epic: &str,
    remembered: Option<&Timestamp>,
) -> Ship {
    if let Some(at) = remembered {
        return day_of(offset, at).map_or(Ship::Unknown, Ship::On);
    }
    match view.and_then(|view| view.epics.iter().find(|one| one.reference == epic)) {
        Some(found) if found.total == 0 || found.done < found.total => Ship::NotYet,
        _ => Ship::Unknown,
    }
}

fn planned_aims<'a>(
    node: &Node,
    definition: &'a agency_proto::OrgDefinition,
    views: &'a BTreeMap<Id, Option<TeamView>>,
    shipped: &BTreeMap<(Id, String), Timestamp>,
) -> Vec<Planned<'a>> {
    let offset = node.utc_offset();
    let mut planned = vec![];
    for place in definition.teams() {
        let Some(plan) = &place.plan else { continue };
        let view = views.get(&place.id).and_then(Option::as_ref);
        for aim in &plan.aims {
            let key = (place.id.clone(), aim.epic.clone());
            let title = view
                .and_then(|view| view.epics.iter().find(|one| one.reference == aim.epic))
                .map(|found| found.title.as_str());
            planned.push(Planned {
                team: place.id.clone(),
                aim: aim.clone(),
                title,
                ship: ship_of(offset, view, &aim.epic, shipped.get(&key)),
            });
        }
    }
    planned
}

/// The reading of `planned[index]` against the KPI's numbers, with the other aims of the same KPI whose ship
/// day is known as the epics it cannot be told apart from.
fn read_one(
    planned: &[Planned<'_>],
    index: usize,
    numbers: &[Observation],
    today: CalendarDate,
) -> AimReading {
    let one = &planned[index];
    let others: Vec<Shipment<'_>> = planned
        .iter()
        .enumerate()
        .filter(|(other_index, other)| *other_index != index && other.aim.kpi == one.aim.kpi)
        .filter_map(|(_, other)| match other.ship {
            Ship::On(on) => Some(Shipment {
                epic: &other.aim.epic,
                on,
            }),
            _ => None,
        })
        .collect();
    let aim = Aim {
        team: &one.team,
        epic: &one.aim.epic,
        epic_title: one.title,
        kpi: &one.aim.kpi,
        ship: one.ship,
    };
    read(&aim, numbers, today, &others)
}

/// A reading for each aim of each team's plan, in the order of the definition. The node first writes down
/// the ship days it sees, so an epic that shipped while nobody read is not lost.
pub async fn readings(node: &Node, org: &Id) -> Result<Readings> {
    let (doc, _) = membership(node, org)?;
    let views = aimed_views(node, &doc.body).await;
    let shipped = remember(node, &doc.body, &views)?;
    let counted = effective(node.store().observations(org));
    let today = node.now().to_offset(node.utc_offset()).date();
    let planned = planned_aims(node, &doc.body, &views, &shipped);
    let readings = planned
        .iter()
        .enumerate()
        .map(|(index, one)| {
            let numbers: Vec<Observation> = counted
                .values()
                .filter(|number| number.kpi == one.aim.kpi)
                .cloned()
                .collect();
            read_one(&planned, index, &numbers, today)
        })
        .collect::<Vec<AimReading>>();
    let generated_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    Ok(Readings {
        org: org.clone(),
        generated_at,
        readings,
    })
}
