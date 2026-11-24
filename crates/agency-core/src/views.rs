//! TeamViews: served for the teams this node hosts, and kept as replicas for teams on other nodes.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs;
use crate::ports::DefinitionAccess;
use crate::read_auth::{sign_read, verify_read, ReadAuth};
use crate::routes;
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::{Id, TeamView};
use serde::{Deserialize, Serialize};

/// A replica of a team on another node. When a read fails, the previous view stays and only the error is recorded.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replica {
    pub fetched_at: Option<String>,
    pub tried_at: String,
    pub error: Option<String>,
    pub view: Option<TeamView>,
}

/// A view is kept only if it is a view of the team that was asked for.
pub fn check_view(view: TeamView, org: &Id, team: &Id) -> Result<TeamView, String> {
    if &view.org != org || &view.team != team {
        return Err(format!(
            "got a view of {}/{} instead of {org}/{team}",
            view.org, view.team
        ));
    }
    Ok(view)
}

/// The TeamView of a team this node hosts, for a signed read by a member of its org.
/// `path` is the path the request was made to, which the signature covers.
pub async fn hosted_view(
    node: &Node,
    org: &Id,
    team: &Id,
    auth: &ReadAuth,
    path: &str,
) -> Result<TeamView> {
    let hosted = node
        .config()
        .hosted(org, team)
        .cloned()
        .ok_or_else(|| Error::not_found("not found"))?;
    let doc = orgs::load(node, org).map_err(|_| Error::not_found("not found"))?;
    verify_read(&doc.body, auth, "GET", path, node.now())?;
    node.runtime()
        .view(&hosted)
        .await
        .map_err(|error| Error::new(ErrorKind::Unavailable, error.message))
}

async fn fetch_view(node: &Node, org: &Id, place: &TeamPlace) -> Result<TeamView, String> {
    let path = routes::view(org, &place.id);
    let auth = sign_read(node.key(), &node.account(), "GET", &path, node.now());
    let view = node
        .peers()
        .get_view(&place.node, &path, &auth)
        .await
        .map_err(|error| error.to_string())?;
    check_view(view, org, &place.id)
}

async fn refresh_replica(node: &Node, org: &Id, place: &TeamPlace) -> Result<()> {
    let previous = node
        .store()
        .load_replica(org, &place.id)
        .unwrap_or_default();
    let now = node.now_text();
    let next = match fetch_view(node, org, place).await {
        Ok(view) => Replica {
            fetched_at: Some(now.clone()),
            tried_at: now,
            error: None,
            view: Some(view),
        },
        Err(error) => Replica {
            tried_at: now,
            error: Some(error),
            ..previous
        },
    };
    node.store().save_replica(org, &place.id, &next)
}

/// Takes a newer org definition from one of the org's other nodes, if it has one.
async fn refresh_definition(node: &Node, doc: &SignedOrg) {
    let me = node.url();
    let Some(place) = doc.body.teams.iter().find(|place| place.node != me) else {
        return;
    };
    let path = routes::definition(&doc.body.org);
    let access = DefinitionAccess::Signed(sign_read(
        node.key(),
        &node.account(),
        "GET",
        &path,
        node.now(),
    ));
    if let Ok(newer) = node
        .peers()
        .get_definition(&place.node, &path, &access)
        .await
    {
        let _ = orgs::receive(node, newer).await;
    }
}

/// Re-reads the replicas of every team on other nodes in the orgs this node's account belongs to.
pub async fn refresh(node: &Node) -> Result<()> {
    let me = node.url();
    let account = node.account();
    for doc in node
        .store()
        .orgs()
        .into_iter()
        .filter(|doc| doc.body.member(&account).is_some())
    {
        for place in doc.body.teams.iter().filter(|place| place.node != me) {
            refresh_replica(node, &doc.body.org, place).await?;
        }
        refresh_definition(node, &doc).await;
    }
    Ok(())
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
