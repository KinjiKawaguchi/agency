//! A team's entry in an org's definition: its label, and whether it is there at all. Like everything in
//! the definition, only the root key's node changes them, by signing the next version (`orgs::amend`).
//! Where a team is (`node`) and its department are changed by `orgs::move_team` and
//! `departments::place_team`.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::amend;
use agency_proto::org::SignedOrg;
use agency_proto::Id;

fn not_in_org(team: &Id, org: &Id) -> Error {
    Error::not_found(format!("team {team} is not in org {org}"))
}

/// Renames a team: the next version, changed only in that team's label. A malformed label fails the
/// definition's own check (`validate`).
pub fn set_label(node: &Node, org: &Id, team: &Id, label: String) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let place = body
            .teams
            .iter_mut()
            .find(|place| &place.id == team)
            .ok_or_else(|| not_in_org(team, org))?;
        if place.label == label {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("team {team} already has that label"),
            ));
        }
        place.label = label;
        Ok(())
    })
}

/// Removes a team from the org's definition: the next version, without it. It refuses while the team is
/// still in this node's own configuration, so a team that runs here is never left without a place in the
/// org; the team is taken out of `config.json` first.
pub fn remove(node: &Node, org: &Id, team: &Id) -> Result<SignedOrg> {
    if node.config().hosted(org, team).is_some() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!(
                "team {org}/{team} is still in this node's config.json; remove it from config.json first, so it is not left running outside the org"
            ),
        ));
    }
    amend(node, org, |body| {
        let before = body.teams.len();
        body.teams.retain(|place| &place.id != team);
        if body.teams.len() == before {
            return Err(not_in_org(team, org));
        }
        Ok(())
    })
}
