//! What a screen draws of an org's structure (the design is on #674, section 5): the
//! units with their parents, heads and teams, the people and agents in them, and who reports to whom. The node
//! works all of it out and a screen only draws it, so the web client and the apps draw the same chart:
//! the nodes of the chart are people and agents, a line means "reports to", units are areas that nest, and an
//! agent or a person that heads several units is in each, marked.
//!
//! Nothing here is authority: a head decides nothing. It holds labels and ids only: no key, no account, no
//! device, no address.
use crate::ids::Id;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a member of the chart is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChartMemberKind {
    /// A member of the org with an account (a person).
    Person,
    /// An agent: a participant that has no account, no key and no devices.
    Agent,
}

/// The team that runs a unit: where, as the definition says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChartTeam {
    /// The origin of the node the team runs on.
    pub node: String,
}

/// How many people and agents are in a unit and in every unit under it, so that a screen that folds a unit shows
/// the count without adding up the units it hides. A screen that wants who sits in the unit itself counts the
/// `members` whose `unit` is its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChartHolds {
    pub people: u32,
    pub agents: u32,
}

/// Who a team's harness says leads it, when that is not who the definition names as the unit's head. The
/// definition wins and the chart draws its head; this is for a screen to mark "the team's harness says
/// otherwise".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ChartRuntimeLead {
    /// The lead as the team's runtime names them.
    pub name: String,
    /// Their id in the org, when the team's `people` map names it. Left out when the runtime's lead is no one
    /// the org knows by that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<Id>,
}

/// A unit of the org: an area of the chart, with a team when a harness runs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ChartUnit {
    pub id: Id,
    pub label: String,
    /// The unit it sits in. Left out for a unit at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Id>,
    /// The person or agent that heads it (`ChartMember::id`). Left out when the unit names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<Id>,
    /// Set when a harness runs the unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<ChartTeam>,
    /// The people and agents in this unit and every unit under it.
    pub holds: ChartHolds,
    /// How many requests wait for an answer in the teams of this unit and of every unit under it: the sum of each
    /// team's whole view (the same for every viewer), as this node knows it (the view it serves, or the replica it
    /// keeps of a team on another node). On a team it is that team's own, with the teams under it when it has any.
    /// Left out when no unit in the subtree has a team, and unless the node has read the view of **every** team in
    /// the subtree: a number on a screen is always the node's, never a part of it and never 0 for "not known".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<u32>,
    /// Left out unless the team's runtime names a lead who is not the unit's head (or the unit names none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_lead: Option<ChartRuntimeLead>,
}

/// A person or an agent on the chart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ChartMember {
    pub id: Id,
    pub label: String,
    pub kind: ChartMemberKind,
    /// The unit it sits in. Left out for a person the org put in no unit; a screen draws them at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Id>,
    /// Who they report to (`ChartMember::id`), worked out from the heads: the head of their unit, and a head
    /// reports to the nearest head above its unit, then the org's head. Left out for the org's head and for
    /// anyone with no head above them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reports_to: Option<Id>,
    /// The units (`ChartUnit::id`) they head, in the order of `units`. Always there, `[]` when they head none, so
    /// a screen decodes it as required.
    #[serde(default)]
    pub heads: Vec<Id>,
}

/// `GET /api/orgs/<org>/chart`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct OrgChart {
    pub org: Id,
    pub label: String,
    /// The version of the org definition the chart is worked out from.
    #[ts(type = "number")]
    pub version: u64,
    /// The org's head (`ChartMember::id`): the person or agent the org names, else the root's person. Left out
    /// when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<Id>,
    /// Every unit, a parent before the units in it.
    pub units: Vec<ChartUnit>,
    /// Every person and agent drawn: not a seat, an external party or a node account.
    pub members: Vec<ChartMember>,
}

impl OrgChart {
    /// Why this is not a chart a screen can draw, in words that name the part: a reference that names no unit or
    /// member, a unit before its parent, an id used twice, a head that is not a member, a `reportsTo` or `heads`
    /// that disagree with the units' heads. A node never answers one that fails; a screen reads one that does
    /// not without checking it again.
    pub fn check(&self) -> Result<(), String> {
        let mut unit_at = std::collections::HashMap::new();
        for (index, unit) in self.units.iter().enumerate() {
            if unit_at.insert(&unit.id, index).is_some() {
                return Err(format!("unit {} is listed twice", unit.id));
            }
            if let Some(parent) = &unit.parent {
                match unit_at.get(parent) {
                    Some(at) if *at < index => {}
                    _ => return Err(format!("unit {} comes before its parent {parent}", unit.id)),
                }
            }
            if unit.runtime_lead.is_some() && unit.team.is_none() {
                return Err(format!(
                    "unit {} has no team and names its runtime's lead",
                    unit.id
                ));
            }
        }
        let mut member_at = std::collections::HashMap::new();
        for member in &self.members {
            if member_at.insert(&member.id, member).is_some() {
                return Err(format!("{} is listed twice", member.id));
            }
        }
        let known_member = |id: &Id, what: &str| {
            member_at
                .contains_key(id)
                .then_some(())
                .ok_or_else(|| format!("{what} {id} is no one on the chart"))
        };
        if let Some(head) = &self.head {
            known_member(head, "the org's head")?;
        }
        for unit in &self.units {
            if let Some(head) = &unit.head {
                known_member(head, &format!("the head of unit {}", unit.id))?;
            }
        }
        for member in &self.members {
            if let Some(unit) = &member.unit {
                if !unit_at.contains_key(unit) {
                    return Err(format!(
                        "{} sits in unit {unit}, which is not listed",
                        member.id
                    ));
                }
            }
            if let Some(boss) = &member.reports_to {
                known_member(boss, &format!("who {} reports to", member.id))?;
                if boss == &member.id {
                    return Err(format!("{} reports to themselves", member.id));
                }
            }
            let headed: Vec<&Id> = self
                .units
                .iter()
                .filter(|unit| unit.head.as_ref() == Some(&member.id))
                .map(|unit| &unit.id)
                .collect();
            if headed.iter().copied().ne(member.heads.iter()) {
                return Err(format!(
                    "{} heads {:?} and the units name them head of {headed:?}",
                    member.id, member.heads
                ));
            }
            let expected = self.reports_to(member, &unit_at);
            if member.reports_to.as_ref() != expected {
                return Err(format!(
                    "{} reports to {:?} and the heads say {expected:?}",
                    member.id, member.reports_to
                ));
            }
        }
        self.check_totals(&unit_at)
    }

    /// Who `member` reports to by the rule of the design: the head of their unit, and for that unit's head the
    /// nearest head above it, then the org's head; a unit with no head passes its people up, and the org's head
    /// reports to no one. A head that is the member themselves is passed over. Because it goes up the units
    /// only, what it gives can never loop.
    fn reports_to(
        &self,
        member: &ChartMember,
        unit_at: &std::collections::HashMap<&Id, usize>,
    ) -> Option<&Id> {
        let mut at = member
            .unit
            .as_ref()
            .and_then(|unit| unit_at.get(unit))
            .copied();
        while let Some(index) = at {
            let unit = &self.units[index];
            if let Some(head) = unit.head.as_ref().filter(|head| *head != &member.id) {
                return Some(head);
            }
            at = unit
                .parent
                .as_ref()
                .and_then(|parent| unit_at.get(parent))
                .copied();
        }
        self.head.as_ref().filter(|head| *head != &member.id)
    }
}

impl OrgChart {
    /// `holds` and `waiting` of a unit are the totals of its subtree: the members whose unit is in it, and the
    /// teams' counts, which a unit has only when every team in it has one.
    fn check_totals(&self, index: &std::collections::HashMap<&Id, usize>) -> Result<(), String> {
        let count = self.units.len();
        let mut holds = vec![(0u32, 0u32); count];
        for member in &self.members {
            let Some(at) = member.unit.as_ref().and_then(|unit| index.get(unit)) else {
                continue;
            };
            match member.kind {
                ChartMemberKind::Person => holds[*at].0 += 1,
                ChartMemberKind::Agent => holds[*at].1 += 1,
            }
        }
        // What the units below a unit say of their teams: whether any has one, and the sum when each has its count.
        let mut below_has_team = vec![false; count];
        let mut below_sum: Vec<Option<u32>> = vec![Some(0); count];
        // A parent is listed before the units in it, so going backwards meets every unit after the units in it.
        for at in (0..count).rev() {
            let unit = &self.units[at];
            let expected = ChartHolds {
                people: holds[at].0,
                agents: holds[at].1,
            };
            if unit.holds != expected {
                return Err(format!(
                    "unit {} holds {:?} and its members and the units under it add up to {expected:?}",
                    unit.id, unit.holds
                ));
            }
            let has_team = unit.team.is_some() || below_has_team[at];
            let is_wrong = match (&unit.team, below_sum[at]) {
                (None, sum) => unit.waiting != if has_team { sum } else { None },
                (Some(_), None) => unit.waiting.is_some(),
                (Some(_), Some(sum)) => unit.waiting.is_some_and(|waiting| waiting < sum),
            };
            if is_wrong {
                return Err(format!(
                    "unit {} counts {:?} waiting, which is not what the teams in it add up to",
                    unit.id, unit.waiting
                ));
            }
            if let Some(parent) = unit.parent.as_ref().and_then(|parent| index.get(parent)) {
                holds[*parent].0 += holds[at].0;
                holds[*parent].1 += holds[at].1;
                if has_team {
                    below_has_team[*parent] = true;
                    below_sum[*parent] = below_sum[*parent]
                        .zip(unit.waiting)
                        .map(|(sum, own)| sum + own);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../../testdata/orgchart.sample.json");

    fn sample() -> OrgChart {
        serde_json::from_str(SAMPLE).unwrap()
    }

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn the_sample_round_trips_and_is_a_chart_a_screen_can_draw() {
        let again = serde_json::to_value(sample()).unwrap();
        assert_eq!(
            again,
            serde_json::from_str::<serde_json::Value>(SAMPLE).unwrap()
        );
        assert_eq!(sample().check(), Ok(()));
    }

    #[test]
    fn the_sample_shows_each_part_a_screen_draws() {
        let chart = sample();
        let unit = |name: &str| chart.units.iter().find(|one| one.id == id(name)).unwrap();
        let member = |name: &str| chart.members.iter().find(|one| one.id == id(name)).unwrap();
        assert!(unit("product").parent.is_none(), "a unit at the top");
        assert!(unit("studio").head.is_none(), "a unit that names no head");
        assert_eq!(
            unit("dev").waiting,
            Some(7),
            "a unit adds up the teams in it"
        );
        assert_eq!(
            unit("app").waiting,
            Some(5),
            "a team counts the teams under it too"
        );
        assert_eq!(unit("app-ios").waiting, Some(2), "a team under a team");
        assert_eq!(
            unit("product").waiting,
            None,
            "and has none while a team in it is not read"
        );
        assert_eq!(
            unit("labs").waiting,
            None,
            "a team whose view is not read yet"
        );
        assert_eq!(unit("studio").waiting, None, "no team in it");
        assert_eq!(
            unit("dev").holds,
            ChartHolds {
                people: 1,
                agents: 2
            },
            "who is under it too"
        );
        assert!(
            unit("web").runtime_lead.is_some(),
            "a runtime that names another lead"
        );
        assert_eq!(
            member("dev-lead").heads.len(),
            2,
            "one that heads several units"
        );
        assert_eq!(member("dev-lead").kind, ChartMemberKind::Agent);
        assert!(
            member("alice").reports_to.is_none(),
            "the org's head reports to no one"
        );
        assert!(
            chart.members.iter().any(|one| one.unit.is_none()),
            "someone in no unit"
        );
    }

    #[test]
    fn a_chart_that_disagrees_with_itself_is_refused_naming_the_part() {
        let broke = |edit: &dyn Fn(&mut OrgChart)| {
            let mut chart = sample();
            edit(&mut chart);
            chart.check().unwrap_err()
        };
        let unit = |chart: &mut OrgChart, name: &str| {
            let at = chart.units.iter().position(|one| one.id == id(name));
            at.unwrap()
        };
        let member = |chart: &mut OrgChart, name: &str| {
            let at = chart.members.iter().position(|one| one.id == id(name));
            at.unwrap()
        };
        assert!(broke(&|c| c.units.swap(0, 1)).contains("before its parent"));
        assert!(broke(&|c| c.units.push(c.units[0].clone())).contains("listed twice"));
        assert!(broke(&|c| c.units[2].head = Some(id("nobody"))).contains("no one on the chart"));
        assert!(broke(&|c| {
            let at = member(c, "dev-lead");
            c.members[at].heads.clear();
        })
        .contains("heads"));
        assert!(broke(&|c| c.members[0].unit = Some(id("nowhere"))).contains("not listed"));
        // Who reports to whom is the heads' word, not a screen's or a node's guess.
        assert!(
            broke(&|c| {
                let at = member(c, "carol");
                c.members[at].reports_to = Some(id("bob"));
            })
            .contains("reports to"),
            "a boss who is not the unit's head"
        );
        assert!(
            broke(&|c| {
                let at = member(c, "alice");
                c.members[at].reports_to = Some(id("dev-lead"));
            })
            .contains("reports to"),
            "the org's head reports to no one"
        );
        assert!(
            broke(&|c| {
                let (dev, app) = (member(c, "dev-lead"), member(c, "app-lead"));
                c.members[dev].reports_to = Some(c.members[app].id.clone());
            })
            .contains("reports to"),
            "a loop of two"
        );
        assert!(
            broke(&|c| {
                let at = member(c, "dev-lead");
                c.members[at].reports_to = Some(id("dev-lead"));
            })
            .contains("themselves"),
            "no one reports to themselves"
        );
        assert!(
            broke(&|c| {
                let at = member(c, "bob");
                c.members[at].reports_to = None;
            })
            .contains("reports to"),
            "a unit with no head passes its people up"
        );
        assert!(broke(&|c| c.members[1].reports_to = Some(id("nobody")))
            .contains("no one on the chart"));
        // What the units hold and what waits in them are the totals of what is under them.
        assert!(
            broke(&|c| {
                let at = unit(c, "studio");
                c.units[at].waiting = Some(1);
            })
            .contains("waiting"),
            "a count where no team is"
        );
        assert!(
            broke(&|c| {
                let at = unit(c, "product");
                c.units[at].waiting = Some(9);
            })
            .contains("waiting"),
            "a count over a team not read"
        );
        assert!(
            broke(&|c| {
                let at = unit(c, "dev");
                c.units[at].waiting = Some(4);
            })
            .contains("waiting"),
            "a sum that is not the teams'"
        );
        assert!(
            broke(&|c| {
                // Only this unit is wrong: the unit above adds up what it says.
                let (app, dev) = (unit(c, "app"), unit(c, "dev"));
                c.units[app].waiting = Some(1);
                c.units[dev].waiting = Some(3);
            })
            .contains("unit app counts"),
            "a team that counts less than the teams under it"
        );
        assert!(
            broke(&|c| {
                let at = unit(c, "app");
                c.units[at].waiting = None;
            })
            .contains("waiting"),
            "a unit that counts over a team of its own that was read"
        );
        assert!(broke(&|c| {
            let at = unit(c, "dev");
            c.units[at].holds.agents = 9;
        })
        .contains("holds"));
    }
}
