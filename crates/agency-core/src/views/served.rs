//! The TeamView of a team this node hosts: read from its runtime, checked, and served to a member or an answerer.
use super::addressing::{answerer_view, answers_for, viewer_name};
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::read_auth::{verify_read_once, ReadAuth};
use crate::unseen;
use agency_proto::view::FederatedView;
use agency_proto::{Id, Role, TeamView};

/// A view is served or kept only if it is a view of the team that was asked for, within the limits of
/// a TeamView (`agency_proto::view::limits`). Both a runtime's view and another node's go through here.
pub fn check_view(view: TeamView, org: &Id, team: &Id) -> Result<TeamView, String> {
    if &view.org != org || &view.team != team {
        return Err(format!(
            "got a view of {}/{} instead of {org}/{team}",
            view.org, view.team
        ));
    }
    view.check_limits()
        .map_err(|over| format!("sent a view beyond the limits: {over}"))?;
    Ok(view)
}

/// The TeamView of a team this node hosts, read from its runtime.
pub(crate) async fn runtime_view(node: &Node, hosted: &HostedTeam) -> Result<TeamView> {
    node.runtime()
        .view(hosted)
        .await
        .map_err(|error| Error::new(ErrorKind::Unavailable, error.message))
}

/// The TeamView of a team this node hosts, for a signed read by a member of its org, with the reader's
/// name in the team. `path` is the path the request was made to, which the signature covers.
pub async fn hosted_view(
    node: &Node,
    org: &Id,
    team: &Id,
    auth: &ReadAuth,
    path: &str,
) -> Result<FederatedView> {
    let hosted = node.hosted(org, team);
    let doc = node.store().load_org(org);
    let definition = doc.as_ref().map(|doc| &doc.body);
    let reader = verify_read_once(node, definition, hosted.is_some(), auth, "GET", path)?;
    let hosted =
        hosted.ok_or_else(|| unseen::refused("a read of a team that does not run here"))?;
    let view = runtime_view(node, &hosted).await?;
    let view = match (reader.role, definition) {
        (Role::Member, _) => view,
        (Role::Answerer, Some(definition)) => answerer_view(view, &hosted, &reader.id, definition),
        (Role::Answerer, None) => {
            return Err(unseen::refused("a read in an org this node does not have"))
        }
    };
    let answers_for = match definition {
        Some(definition) => answers_for(definition, &hosted, &reader.id),
        None => vec![],
    };
    Ok(FederatedView {
        view,
        viewer: viewer_name(&hosted, &reader.id),
        answers_for,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::Timestamp;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn view(org: &str, team: &str) -> TeamView {
        let at = Timestamp::parse("2026-11-24T12:00:00Z").unwrap();
        TeamView {
            org: id(org),
            team: id(team),
            label: "App".into(),
            generated_at: at,
            people: vec![],
            requests: vec![],
            work: vec![],
            epics: vec![],
        }
    }

    #[test]
    fn keeps_only_a_view_of_the_team_asked_for() {
        assert!(check_view(view("acme", "app"), &id("acme"), &id("app")).is_ok());
        assert!(check_view(view("acme", "app"), &id("acme"), &id("other")).is_err());
        assert!(check_view(view("acme", "app"), &id("other"), &id("app")).is_err());
    }
}
