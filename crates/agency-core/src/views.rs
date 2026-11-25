//! TeamViews: served for the teams this node hosts, and kept as replicas for teams on other nodes.
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs;
use crate::ports::DefinitionAccess;
use crate::read_auth::{sign_read, verify_read, ReadAuth};
use crate::routes;
use crate::unseen;
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::view::{FederatedView, Request, WaitingOn};
use agency_proto::{Id, Role, TeamView};
use serde::{Deserialize, Serialize};

/// A replica of a team on another node. When a read fails, the previous view stays and only the error is recorded.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replica {
    pub fetched_at: Option<String>,
    pub tried_at: String,
    pub error: Option<String>,
    pub view: Option<TeamView>,
    /// This node's account's name in the team runtime, as the team's node reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
}

/// A person's name in a hosted team's runtime: their entry in `people`, or `owner` for the team's owner.
/// `None` when the team does not address them.
pub fn viewer_name(hosted: &HostedTeam, person: &Id) -> Option<String> {
    let mapped = hosted
        .people
        .iter()
        .find(|(_, id)| *id == person)
        .map(|(name, _)| name.clone());
    mapped.or_else(|| (&hosted.owner == person).then(|| "owner".to_owned()))
}

/// Who reads a team's requests, as this node knows them.
pub enum Reader<'a> {
    /// A team on this node: its hosted-team configuration maps runtime names to org people.
    Hosted {
        hosted: &'a HostedTeam,
        person: &'a Id,
    },
    /// A team on another node: the reader's runtime name there, as that node reported it.
    Remote { viewer: Option<&'a str> },
}

/// The one rule for which requests are the reader's to decide (docs/protocol.md, "Requests"): on a team
/// here, the request's addressee is the reader (an absent or unmapped `to` goes to the team's owner); on
/// a team elsewhere, `to` is the reader's runtime name, or it is absent and that name is `owner`.
pub fn is_addressed_to(reader: &Reader, request: &Request) -> bool {
    let to = request.to.as_deref();
    match reader {
        Reader::Hosted { hosted, person } => hosted.addressee(to) == *person,
        Reader::Remote {
            viewer: Some(viewer),
        } => to.unwrap_or("owner") == *viewer,
        Reader::Remote { viewer: None } => false,
    }
}

/// The ids of the requests in `view` that are the reader's to decide.
pub fn mine(view: &TeamView, reader: &Reader) -> Vec<String> {
    let requests = view.requests.iter();
    requests
        .filter(|request| is_addressed_to(reader, request))
        .map(|request| request.id.clone())
        .collect()
}

/// The people in `view` who can only continue once the viewer acts: their `waitingOn` names the viewer's
/// runtime name. Unlike a request's `to`, an unknown name is not the owner's, because `waitingOn` can name
/// anyone on the team, agents included. A viewer the team does not address has none.
pub fn waiting_on_viewer(view: &TeamView, viewer: Option<&str>) -> Vec<String> {
    let Some(viewer) = viewer else {
        return vec![];
    };
    let waiting = view.people.iter().filter(|person| {
        matches!(&person.waiting_on, Some(WaitingOn::Person { person: name }) if name == viewer)
    });
    waiting.map(|person| person.id.clone()).collect()
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
    let hosted = node.config().hosted(org, team).cloned();
    let doc = node.store().load_org(org).filter(|_| hosted.is_some());
    let reader = verify_read(
        doc.as_ref().map(|doc| &doc.body),
        auth,
        "GET",
        path,
        node.now(),
    )?;
    let hosted = hosted.ok_or_else(unseen::not_found)?;
    let view = runtime_view(node, &hosted).await?;
    let view = match reader.role {
        Role::Member => view,
        Role::Answerer => answerer_view(view, &hosted, &reader.id),
    };
    Ok(FederatedView {
        view,
        viewer: viewer_name(&hosted, &reader.id),
    })
}

/// What an answerer reads of a team: only the requests addressed to them, which they need to answer,
/// and nothing of the team's people, work, or epics. A member reads it all.
pub fn answerer_view(view: TeamView, hosted: &HostedTeam, person: &Id) -> TeamView {
    let reader = Reader::Hosted { hosted, person };
    let requests = view.requests.into_iter();
    TeamView {
        requests: requests
            .filter(|request| is_addressed_to(&reader, request))
            .collect(),
        people: vec![],
        work: vec![],
        epics: vec![],
        ..view
    }
}

async fn fetch_view(node: &Node, org: &Id, place: &TeamPlace) -> Result<FederatedView, String> {
    let path = routes::view(org, &place.id);
    let auth = sign_read(node.key(), &node.account(), "GET", &path, node.now());
    let fetched = node
        .peers()
        .get_view(&place.node, &path, &auth)
        .await
        .map_err(|error| error.to_string())?;
    let view = check_view(fetched.view, org, &place.id)?;
    Ok(FederatedView {
        view,
        viewer: fetched.viewer,
    })
}

async fn refresh_replica(node: &Node, org: &Id, place: &TeamPlace) -> Result<()> {
    let previous = node
        .store()
        .load_replica(org, &place.id)
        .unwrap_or_default();
    let now = node.now_text();
    let next = match fetch_view(node, org, place).await {
        Ok(fetched) => Replica {
            fetched_at: Some(now.clone()),
            tried_at: now,
            error: None,
            view: Some(fetched.view),
            viewer: fetched.viewer,
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
    let org = &doc.body.org;
    let Some(place) = doc
        .body
        .teams
        .iter()
        .find(|place| !node.runs_here(org, &place.id))
    else {
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
    let account = node.account();
    for doc in node
        .store()
        .orgs()
        .into_iter()
        .filter(|doc| doc.body.member(&account).is_some())
    {
        let org = &doc.body.org;
        let elsewhere = doc.body.teams.iter();
        for place in elsewhere.filter(|place| !node.runs_here(org, &place.id)) {
            refresh_replica(node, org, place).await?;
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
    fn viewers_are_named_by_the_people_map_then_as_owner() {
        use crate::config::RuntimeCommand;
        let hosted = HostedTeam {
            org: id("acme"),
            team: id("app"),
            owner: id("alice"),
            people: [("reviewer".to_owned(), id("bob"))].into(),
            runtime: RuntimeCommand {
                command: vec![],
                env: Default::default(),
            },
        };
        assert_eq!(viewer_name(&hosted, &id("alice")).as_deref(), Some("owner"));
        assert_eq!(
            viewer_name(&hosted, &id("bob")).as_deref(),
            Some("reviewer")
        );
        assert_eq!(viewer_name(&hosted, &id("carol")), None);
        let renamed = HostedTeam {
            people: [("kin".to_owned(), id("alice"))].into(),
            ..hosted
        };
        assert_eq!(
            viewer_name(&renamed, &id("alice")).as_deref(),
            Some("kin"),
            "a mapped name wins over owner"
        );
    }

    fn request(to: Option<&str>) -> Request {
        use agency_proto::view::{AnswerWith, RequestKind};
        Request {
            id: "r-1".into(),
            kind: RequestKind::Question,
            title: "Make a key?".into(),
            context: String::new(),
            document: None,
            from: "lead".into(),
            to: to.map(str::to_owned),
            created_at: Timestamp::parse("2026-11-24T12:00:00Z").unwrap(),
            issue: None,
            blocks: vec![],
            options: vec![],
            thread: None,
            answer_with: AnswerWith {
                kind: agency_proto::EnvelopeKind::Answer,
                target: "r-1".into(),
            },
        }
    }

    fn person(id: &str, waiting_on: Option<WaitingOn>) -> agency_proto::view::PersonStatus {
        use agency_proto::view::{PersonKind, PersonRole, PersonState, PersonStatus};
        PersonStatus {
            id: id.into(),
            label: id.into(),
            kind: PersonKind::Agent,
            role: PersonRole::Member,
            state: PersonState::Working,
            since: None,
            waiting_on,
            status: String::new(),
            holding: vec![],
        }
    }

    #[test]
    fn people_wait_on_the_viewer_only_by_the_viewers_own_name() {
        let on = |name: &str| {
            Some(WaitingOn::Person {
                person: name.into(),
            })
        };
        let team = TeamView {
            people: vec![
                person("backend", on("owner")),
                person("frontend", on("reviewer")),
                person("tester", on("backend")),
                person("writer", Some(WaitingOn::Lead)),
                person("lead", None),
            ],
            ..view("acme", "app")
        };
        assert_eq!(waiting_on_viewer(&team, Some("owner")), vec!["backend"]);
        assert_eq!(waiting_on_viewer(&team, Some("reviewer")), vec!["frontend"]);
        assert!(
            waiting_on_viewer(&team, None).is_empty(),
            "a viewer the team does not address"
        );
    }

    #[test]
    fn a_request_is_mine_by_one_rule() {
        use crate::config::RuntimeCommand;
        let hosted = HostedTeam {
            org: id("acme"),
            team: id("app"),
            owner: id("alice"),
            people: [("reviewer".to_owned(), id("bob"))].into(),
            runtime: RuntimeCommand {
                command: vec![],
                env: Default::default(),
            },
        };
        let (alice, bob) = (id("alice"), id("bob"));
        let here = |person| Reader::Hosted {
            hosted: &hosted,
            person,
        };
        let cases = [
            ("absent to: the owner's", here(&alice), None, true),
            ("absent to: not the reviewer's", here(&bob), None, false),
            ("a mapped name", here(&bob), Some("reviewer"), true),
            (
                "an unmapped name: the owner's",
                here(&alice),
                Some("someone"),
                true,
            ),
            (
                "an unmapped name: not the reviewer's",
                here(&bob),
                Some("someone"),
                false,
            ),
        ];
        for (name, reader, to, expected) in cases {
            assert_eq!(is_addressed_to(&reader, &request(to)), expected, "{name}");
        }
        let elsewhere = |viewer| Reader::Remote { viewer };
        let cases = [
            (
                "their name",
                elsewhere(Some("reviewer")),
                Some("reviewer"),
                true,
            ),
            (
                "another name",
                elsewhere(Some("reviewer")),
                Some("lead"),
                false,
            ),
            ("absent to, as owner", elsewhere(Some("owner")), None, true),
            (
                "absent to, as reviewer",
                elsewhere(Some("reviewer")),
                None,
                false,
            ),
            ("not addressed by the team", elsewhere(None), None, false),
        ];
        for (name, reader, to, expected) in cases {
            assert_eq!(is_addressed_to(&reader, &request(to)), expected, "{name}");
        }
    }

    #[test]
    fn keeps_only_a_view_of_the_team_asked_for() {
        assert!(check_view(view("acme", "app"), &id("acme"), &id("app")).is_ok());
        assert!(check_view(view("acme", "app"), &id("acme"), &id("other")).is_err());
        assert!(check_view(view("acme", "app"), &id("other"), &id("app")).is_err());
    }
}
