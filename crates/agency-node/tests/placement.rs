//! Where a team runs: the node's own hosted-team configuration decides, while the org's place is only
//! where other nodes reach it. A place that lags behind this node's URL is reported and fixed with
//! `team-move`, and the team keeps running here meanwhile.
mod common;

use agency_core::decisions::Decided;
use agency_core::devices::{add_device, DeviceSigner};
use agency_core::orgs::{misplaced_teams, move_team, Misplaced};
use agency_core::outbox::pending;
use agency_core::signed_decisions::decide_with_device;
use agency_core::views::refresh;
use agency_core::ErrorKind;
use agency_proto::{AccountKey, EnvelopeKind, Payload};
use common::{alice, body, get, id, joined_bob, post, signed_answer, ENVELOPES, TOKEN};
use serde_json::json;

const ELSEWHERE: &str = "http://127.0.0.1:9";

#[tokio::test]
async fn a_hosted_team_runs_here_even_when_its_place_names_another_origin() {
    let (alice, memory) = alice().await;
    move_team(&alice.node, &id("acme"), &id("app"), ELSEWHERE.into()).unwrap();
    assert_eq!(
        misplaced_teams(&alice.node),
        vec![Misplaced {
            org: id("acme"),
            team: id("app"),
            place: Some(ELSEWHERE.into()),
        }]
    );
    assert!(
        pending(&alice.node).is_empty(),
        "a new version is not sent to the place of a team that runs here"
    );

    let me = body(get(&alice, "/api/me").send().await.unwrap()).await.1;
    assert_eq!(me["orgs"][0]["teams"][0]["location"], "local");
    let snapshot = body(
        get(&alice, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap(),
    )
    .await
    .1;
    assert_eq!(snapshot["source"]["kind"], "local");

    let phone = AccountKey::generate();
    add_device(&alice.node, "Phone", &phone.public(), None).await;
    let decided = body(
        post(
            &alice,
            ENVELOPES,
            &json!({ "envelope": signed_answer(&alice, &phone, "r-1", "a") }),
        )
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap(),
    )
    .await
    .1;
    assert_eq!(decided["outcome"], "applied");
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
    assert!(
        pending(&alice.node).is_empty(),
        "nothing sent to the stale place"
    );

    let signer = phone.public().device_id();
    let device = DeviceSigner {
        id: &signer,
        key: &phone,
    };
    let payload = Payload {
        text: "Go".into(),
        choice: None,
    };
    let (org, team) = (id("acme"), id("app"));
    let decided = decide_with_device(
        &alice.node,
        device,
        &org,
        &team,
        EnvelopeKind::Answer,
        "r-1".into(),
        payload,
    )
    .await
    .unwrap();
    assert_eq!(
        decided,
        Decided::Applied,
        "a decision by the CLI device too"
    );

    memory.lock().unwrap().usage = Some(
        serde_json::from_str(include_str!("../../../testdata/tokenusage.sample.json")).unwrap(),
    );
    let usage = get(&alice, "/api/orgs/acme/teams/app/usage")
        .send()
        .await
        .unwrap();
    assert_eq!(usage.status(), 200, "usage is read from the runtime here");

    refresh(&alice.node).await.unwrap();
    assert!(
        alice
            .node
            .store()
            .load_replica(&id("acme"), &id("app"))
            .is_none(),
        "no replica of a team that runs here"
    );
}

#[tokio::test]
async fn team_move_changes_only_that_teams_place_on_the_roots_node() {
    let (alice, _) = alice().await;
    let before = alice.node.store().load_org(&id("acme")).unwrap();
    let moved = move_team(&alice.node, &id("acme"), &id("app"), ELSEWHERE.into()).unwrap();
    assert_eq!(moved.body.version, before.body.version + 1);
    let mut expected = before.body.teams.clone();
    expected[0].node = ELSEWHERE.into();
    assert_eq!(moved.body.teams, expected);
    assert_eq!(moved.body.people, before.body.people);

    let refusals = [
        (
            "not an origin",
            "app",
            "https://alice.example/path",
            ErrorKind::BadRequest,
        ),
        ("unknown team", "web", ELSEWHERE, ErrorKind::NotFound),
        ("already there", "app", ELSEWHERE, ErrorKind::Conflict),
    ];
    for (name, team, origin, kind) in refusals {
        let error = move_team(&alice.node, &id("acme"), &id(team), origin.into()).unwrap_err();
        assert_eq!(error.kind, kind, "{name}");
    }
    let bob = joined_bob(&alice).await;
    let error = move_team(&bob.node, &id("acme"), &id("app"), bob.url()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Forbidden, "only the root's node");

    move_team(&alice.node, &id("acme"), &id("app"), alice.url()).unwrap();
    assert_eq!(misplaced_teams(&alice.node), vec![]);
}
