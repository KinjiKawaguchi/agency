//! TeamViews: served for the teams this node hosts, and kept as replicas for teams on other nodes.
use crate::config::HostedTeam;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs;
use crate::peer_about;
use crate::ports::{DefinitionAccess, PeerError};
use crate::read_auth::{sign_read, verify_read_once, ReadAuth};
use crate::routes;
use crate::unseen;
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::view::{AnswersFor, FederatedView, Request, WaitingOn};
use agency_proto::{Id, OrgDefinition, Role, TeamView};
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
    /// The seats and external parties this node's account answers for in the team, as the team's node
    /// reported them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answers_for: Vec<AnswersFor>,
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
    /// A team on this node: its hosted-team configuration maps runtime names to org people, seats, and
    /// external parties, which the org definition says who answers for.
    Hosted {
        hosted: &'a HostedTeam,
        person: &'a Id,
        definition: &'a OrgDefinition,
    },
    /// A team on another node: the reader's runtime name there and the seats and external parties they
    /// answer for there, as that node reported them.
    Remote {
        viewer: Option<&'a str>,
        answers_for: &'a [AnswersFor],
    },
}

impl<'a> Reader<'a> {
    /// The reader of a team on another node, from the last replica of it.
    pub fn of_replica(replica: &'a Replica) -> Self {
        Reader::Remote {
            viewer: replica.viewer.as_deref(),
            answers_for: &replica.answers_for,
        }
    }
}

/// The members who answer a request that is addressed to `addressee` on the team `hosted`: the member
/// itself; for a seat someone took, that member; for a seat, the members in its `relays`, or the team's owner
/// when it has none; for an external party, the members in its `relays`. A seat and an external party answer
/// nothing themselves.
pub fn answerers<'a>(
    definition: &'a OrgDefinition,
    hosted: &'a HostedTeam,
    addressee: &'a Id,
) -> Vec<&'a Id> {
    if let Some(taker) = definition
        .people
        .iter()
        .find(|one| one.formerly.contains(addressee))
    {
        return vec![&taker.id];
    }
    if let Some(seat) = definition.seats.iter().find(|one| &one.id == addressee) {
        return if seat.relays.is_empty() {
            vec![&hosted.owner]
        } else {
            seat.relays.iter().collect()
        };
    }
    if let Some(external) = definition.externals.iter().find(|one| &one.id == addressee) {
        return external.relays.iter().collect();
    }
    vec![addressee]
}

/// The seat or external party a request is addressed to on the team `hosted`, with its id and its label;
/// `None` when it is addressed to a member.
pub fn addressed_party<'a>(
    definition: &'a OrgDefinition,
    hosted: &'a HostedTeam,
    to: Option<&str>,
) -> Option<(&'a Id, &'a str)> {
    let addressee = hosted.addressee(to);
    let seat = definition.seats.iter().find(|one| &one.id == addressee);
    let external = definition.externals.iter().find(|one| &one.id == addressee);
    seat.map(|one| (&one.id, one.label.as_str()))
        .or_else(|| external.map(|one| (&one.id, one.label.as_str())))
}

/// The seat or external party a request is addressed to on a team elsewhere, as the team's node said the
/// reader answers for it, with its id and label as `definition` has it. Only what this node's own definition
/// confirms is said: a seat or external party the org has and whose `relays` name the reader (or a seat with
/// none). `None` otherwise, when `to` names no party the reader answers for, or a seat the reader took. The
/// request may still be the reader's to answer (the team's node decides that); it is then said to be for no one.
pub fn remote_addressed_party<'a>(
    definition: &'a OrgDefinition,
    answers_for: &[AnswersFor],
    reader: &Id,
    to: Option<&str>,
) -> Option<(&'a Id, &'a str)> {
    let answered = answers_for
        .iter()
        .find(|one| Some(one.name.as_str()) == to)?;
    if is_taken_seat(definition, &answered.id) {
        return None;
    }
    // Only what this node's own definition says: a seat or external party of the org that the reader answers
    // for. A seat with no relays is answered by the team's owner, whom only the team's node knows.
    let seat = definition
        .seats
        .iter()
        .find(|one| one.id == answered.id)
        .filter(|one| one.relays.is_empty() || one.relays.contains(reader))
        .map(|one| (&one.id, one.label.as_str()));
    let external = || {
        definition
            .externals
            .iter()
            .find(|one| one.id == answered.id && one.relays.contains(reader))
            .map(|one| (&one.id, one.label.as_str()))
    };
    seat.or_else(external)
}

/// The one rule for which requests are the reader's to decide (docs/protocol.md, "Requests"): on a team
/// here, the reader answers the request when its addressee is the reader, or a seat or external party the
/// reader answers for (an absent or unmapped `to` goes to the team's owner); on a team elsewhere, `to` is
/// the reader's runtime name, or it is absent and that name is `owner`, or it is the name of a seat or
/// external party the team's node says the reader answers for. The team node's check of a decision
/// and every reader's list of their requests both go through it, so no one is shown a request they could
/// not answer, and no one answers one they cannot see.
pub fn is_addressed_to(reader: &Reader, request: &Request) -> bool {
    let to = request.to.as_deref();
    match reader {
        Reader::Hosted {
            hosted,
            person,
            definition,
        } => answerers(definition, hosted, hosted.addressee(to)).contains(person),
        Reader::Remote {
            viewer,
            answers_for,
        } => {
            let is_own = viewer.is_some_and(|viewer| to.unwrap_or("owner") == viewer);
            is_own || answers_for.iter().any(|one| Some(one.name.as_str()) == to)
        }
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

/// Whether `id` is a seat that a member took: it is no seat now, and is in that member's `formerly`.
fn is_taken_seat(definition: &OrgDefinition, id: &Id) -> bool {
    definition
        .people
        .iter()
        .any(|one| one.formerly.contains(id))
}

/// The seats and external parties of the org that the team's `people` name and that `person` answers for
/// on the team `hosted`, by the name the team's runtime knows each by. A seat the member took counts too: the
/// request to it is theirs, so the team's node keeps it in their list.
fn answers_for(definition: &OrgDefinition, hosted: &HostedTeam, person: &Id) -> Vec<AnswersFor> {
    hosted
        .people
        .iter()
        .filter(|(_, id)| {
            let is_party = definition.seats.iter().any(|one| &one.id == *id)
                || definition.externals.iter().any(|one| &one.id == *id)
                || is_taken_seat(definition, id);
            is_party && answerers(definition, hosted, id).contains(&person)
        })
        .map(|(name, id)| AnswersFor {
            name: name.clone(),
            id: id.clone(),
        })
        .collect()
}

/// What an answerer reads of a team: only the requests addressed to them, which they need to answer,
/// and nothing of the team's people, work, or epics. A member reads it all.
pub fn answerer_view(
    view: TeamView,
    hosted: &HostedTeam,
    person: &Id,
    definition: &OrgDefinition,
) -> TeamView {
    let reader = Reader::Hosted {
        hosted,
        person,
        definition,
    };
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

/// Why a team's view could not be read from its node, and whether that node answered 404 (the peer may not
/// be the release this node read it as).
struct FetchFailure {
    message: String,
    is_not_found: bool,
}

async fn fetch_view(
    node: &Node,
    org: &Id,
    place: &TeamPlace,
) -> Result<FederatedView, FetchFailure> {
    let path = routes::view(org, &place.id);
    let auth = sign_read(node.key(), &node.account(), "GET", &path, node.now());
    let fetched = node
        .peers()
        .get_view(&place.node, &path, &auth)
        .await
        .map_err(|error| FetchFailure {
            is_not_found: matches!(error, PeerError::Status { status: 404, .. }),
            message: error.to_string(),
        })?;
    let other = |message: String| FetchFailure {
        message,
        is_not_found: false,
    };
    fetched
        .check_answers_for()
        .map_err(|over| other(format!("sent an answer beyond the limits: {over}")))?;
    let view = check_view(fetched.view, org, &place.id).map_err(other)?;
    Ok(FederatedView {
        view,
        viewer: fetched.viewer,
        answers_for: fetched.answers_for,
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
            answers_for: fetched.answers_for,
        },
        Err(failure) => {
            if failure.is_not_found {
                peer_about::note_not_found(node, org, &place.node)?;
            }
            Replica {
                tried_at: now,
                error: Some(failure.message),
                ..previous
            }
        }
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
        if newer.body.version > doc.body.version + 1 {
            let _ = orgs::catch_up(node, &place.node, org, newer.body.version).await;
        }
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
        peer_about::refresh(node, &doc.body).await?;
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
        let hosted = HostedTeam::new(
            id("acme"),
            id("app"),
            id("alice"),
            RuntimeCommand::of(vec![]),
        )
        .with_people([("reviewer".to_owned(), id("bob"))].into());
        assert_eq!(viewer_name(&hosted, &id("alice")).as_deref(), Some("owner"));
        assert_eq!(
            viewer_name(&hosted, &id("bob")).as_deref(),
            Some("reviewer")
        );
        assert_eq!(viewer_name(&hosted, &id("carol")), None);
        let renamed = hosted.with_people([("kin".to_owned(), id("alice"))].into());
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
        let hosted = HostedTeam::new(
            id("acme"),
            id("app"),
            id("alice"),
            RuntimeCommand::of(vec![]),
        )
        .with_people([("reviewer".to_owned(), id("bob"))].into());
        let (alice, bob) = (id("alice"), id("bob"));
        let definition = definition_with(&["alice", "bob"]);
        let here = |person| Reader::Hosted {
            hosted: &hosted,
            person,
            definition: &definition,
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
        let elsewhere = |viewer| Reader::Remote {
            viewer,
            answers_for: &[],
        };
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
        let answers_for = [AnswersFor {
            name: "finance".into(),
            id: id("carol"),
        }];
        let answering = |viewer| Reader::Remote {
            viewer,
            answers_for: &answers_for,
        };
        let cases = [
            (
                "a seat they answer for, with a name",
                answering(Some("reviewer")),
                Some("finance"),
                true,
            ),
            (
                "a seat they answer for, without a name in the team",
                answering(None),
                Some("finance"),
                true,
            ),
            (
                "a seat they do not answer for",
                answering(Some("reviewer")),
                Some("tax"),
                false,
            ),
            (
                "their own name still",
                answering(Some("reviewer")),
                Some("reviewer"),
                true,
            ),
            (
                "absent to goes to the owner, not to a seat",
                answering(None),
                None,
                false,
            ),
        ];
        for (name, reader, to, expected) in cases {
            assert_eq!(is_addressed_to(&reader, &request(to)), expected, "{name}");
        }
    }

    /// An org of these members, with no seats or external parties yet.
    fn definition_with(members: &[&str]) -> OrgDefinition {
        use agency_proto::org::{Person, Root};
        use agency_proto::AccountKey;
        let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
        let people: Vec<Person> = members
            .iter()
            .enumerate()
            .map(|(n, raw)| {
                Person::new(
                    id(raw),
                    raw.to_uppercase(),
                    id(raw),
                    AccountKey::from_seed([10 + n as u8; 32]).public(),
                )
            })
            .collect();
        OrgDefinition::new(
            id("acme"),
            "Acme".into(),
            root,
            "2026-11-24T12:00:00Z".into(),
        )
        .with_people(people)
    }

    #[test]
    fn a_request_for_a_seat_or_an_external_party_is_the_members_who_answer_for_them() {
        use crate::config::RuntimeCommand;
        use agency_proto::seat::{External, Seat};
        let hosted = HostedTeam::new(
            id("acme"),
            id("app"),
            id("alice"),
            RuntimeCommand::of(vec![]),
        )
        .with_people(
            [
                ("reviewer", "bob"),
                ("finance", "carol"),
                ("tax", "dan"),
                ("counsel", "erin"),
                ("typo", "zed"),
            ]
            .map(|(name, member)| (name.to_owned(), id(member)))
            .into(),
        );
        let mut definition = definition_with(&["alice", "bob", "cara"]);
        let seat = |raw: &str, relays: &[&str]| Seat {
            id: id(raw),
            label: format!("{raw} (seat)"),
            department: None,
            ask_about: vec![],
            relays: relays.iter().map(|one| id(one)).collect(),
        };
        definition.seats = vec![seat("carol", &["bob"]), seat("dan", &[])];
        definition.externals = vec![External {
            id: id("erin"),
            label: "Erin (outside counsel)".into(),
            ask_about: vec![],
            relays: vec![id("alice"), id("cara")],
        }];
        let reads = |person: &str, to: Option<&str>| {
            let person = id(person);
            let reader = Reader::Hosted {
                hosted: &hosted,
                person: &person,
                definition: &definition,
            };
            is_addressed_to(&reader, &request(to))
        };
        // A seat with someone to answer for it: those members, and not the owner.
        assert!(reads("bob", Some("finance")));
        assert!(!reads("alice", Some("finance")) && !reads("cara", Some("finance")));
        // A seat with none: the team's owner.
        assert!(reads("alice", Some("tax")));
        assert!(!reads("bob", Some("tax")));
        // An external party: each of the members named, in any order, and no one else.
        assert!(reads("alice", Some("counsel")) && reads("cara", Some("counsel")));
        assert!(!reads("bob", Some("counsel")));
        // A member addressed directly, and a name that names no one, are what they were.
        assert!(reads("bob", Some("reviewer")) && !reads("alice", Some("reviewer")));
        assert!(reads("alice", None) && reads("alice", Some("unmapped")));
        assert!(
            !reads("alice", Some("typo")) && !reads("bob", Some("typo")),
            "an id that is nothing"
        );

        let party = |to: Option<&str>| {
            addressed_party(&definition, &hosted, to)
                .map(|(id, label)| (id.to_string(), label.to_owned()))
        };
        assert_eq!(
            party(Some("finance")),
            Some(("carol".into(), "carol (seat)".into()))
        );
        assert_eq!(party(Some("counsel")).map(|one| one.0), Some("erin".into()));
        assert_eq!(party(Some("reviewer")), None, "a member is not a party");
        assert_eq!(party(None), None, "the owner is a member");
    }

    #[test]
    fn keeps_only_a_view_of_the_team_asked_for() {
        assert!(check_view(view("acme", "app"), &id("acme"), &id("app")).is_ok());
        assert!(check_view(view("acme", "app"), &id("acme"), &id("other")).is_err());
        assert!(check_view(view("acme", "app"), &id("other"), &id("app")).is_err());
    }
}
