//! What an org measures and what its teams aim at, kept in the signed definition and changed only by its
//! root: KPIs and aims, no numbers.
mod common;

use agency_core::departments::plan_team;
use agency_core::kpis::{self, Change};
use agency_core::ErrorKind;
use agency_proto::org::{Allocation, TeamPlan};
use agency_proto::outcome::{Currency, GoodWhen, Kpi, KpiKind, KpiUnit};
use common::{alice, id, joined_bob, Running};

fn body(running: &Running) -> agency_proto::OrgDefinition {
    running.node.store().load_org(&id("acme")).unwrap().body
}

fn revenue() -> Kpi {
    Kpi {
        id: id("revenue"),
        label: "Revenue".into(),
        kind: KpiKind::Business,
        unit: KpiUnit::Money {
            currency: Currency::parse("JPY").unwrap(),
        },
        good_when: GoodWhen::Higher,
    }
}

fn plan() -> TeamPlan {
    let allocation = Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    };
    TeamPlan::new(allocation, 500)
}

#[tokio::test]
async fn the_root_states_a_kpi_in_one_version() {
    let (alice, _) = alice().await;
    let before = body(&alice).version;
    kpis::add(&alice.node, &id("acme"), revenue()).unwrap();
    let after = body(&alice);
    assert_eq!(after.version, before + 1);
    assert_eq!(after.kpis, vec![revenue()]);

    let twice = kpis::add(&alice.node, &id("acme"), revenue()).unwrap_err();
    assert_eq!(twice.kind, ErrorKind::Conflict);
    assert_eq!(
        body(&alice).version,
        before + 1,
        "a refusal writes no version"
    );
}

#[tokio::test]
async fn a_kpis_label_unit_and_direction_change_and_nothing_else() {
    let (alice, _) = alice().await;
    kpis::add(&alice.node, &id("acme"), revenue()).unwrap();
    let change = Change {
        label: Some("Monthly revenue".into()),
        unit: Some(KpiUnit::Money {
            currency: Currency::parse("USD").unwrap(),
        }),
        good_when: Some(GoodWhen::Lower),
    };
    kpis::change(&alice.node, &id("acme"), &id("revenue"), change).unwrap();
    let kpi = body(&alice).kpis[0].clone();
    assert_eq!(kpi.label, "Monthly revenue");
    assert_eq!(kpi.good_when, GoodWhen::Lower);
    assert_eq!((kpi.id, kpi.kind), (id("revenue"), KpiKind::Business));

    let same = Change {
        label: Some("Monthly revenue".into()),
        ..Change::default()
    };
    let error = kpis::change(&alice.node, &id("acme"), &id("revenue"), same).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    let missing = kpis::change(&alice.node, &id("acme"), &id("none"), Change::default());
    assert_eq!(missing.unwrap_err().kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn an_aim_needs_a_plan_and_a_kpi_and_is_not_repeated() {
    let (alice, _) = alice().await;
    let (org, team, kpi) = (id("acme"), id("app"), id("revenue"));
    let aim =
        |epic: &str, kpi: &agency_proto::Id| kpis::aim(&alice.node, &org, &team, epic.into(), kpi);
    assert_eq!(
        aim("app#1", &kpi).unwrap_err().kind,
        ErrorKind::NotFound,
        "no such KPI"
    );
    kpis::add(&alice.node, &org, revenue()).unwrap();
    let no_plan = aim("app#1", &kpi).unwrap_err();
    assert_eq!(no_plan.kind, ErrorKind::Conflict);
    assert!(
        no_plan.message.contains("team-plan"),
        "names the fix: {}",
        no_plan.message
    );

    plan_team(&alice.node, &org, &team, Some(plan())).unwrap();
    aim("app#1", &kpi).unwrap();
    assert_eq!(
        aim("app#1", &kpi).unwrap_err().kind,
        ErrorKind::Conflict,
        "repeated"
    );
    aim("app#2", &kpi).unwrap();
    let aims = body(&alice).team(&team).unwrap().plan.clone().unwrap().aims;
    assert_eq!(aims.len(), 2);

    kpis::unaim(&alice.node, &org, &team, "app#1", &kpi).unwrap();
    let gone = kpis::unaim(&alice.node, &org, &team, "app#1", &kpi).unwrap_err();
    assert_eq!(gone.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn a_kpi_a_plan_aims_at_is_not_removed() {
    let (alice, _) = alice().await;
    let (org, team, kpi) = (id("acme"), id("app"), id("revenue"));
    kpis::add(&alice.node, &org, revenue()).unwrap();
    plan_team(&alice.node, &org, &team, Some(plan())).unwrap();
    kpis::aim(&alice.node, &org, &team, "app#1".into(), &kpi).unwrap();
    let version = body(&alice).version;

    let refused = kpis::remove(&alice.node, &org, &kpi).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(
        refused.message.contains("team app") && refused.message.contains("plan-unaim"),
        "names the team and the fix: {}",
        refused.message
    );
    assert_eq!(body(&alice).version, version);

    kpis::unaim(&alice.node, &org, &team, "app#1", &kpi).unwrap();
    kpis::remove(&alice.node, &org, &kpi).unwrap();
    assert!(body(&alice).kpis.is_empty());
    assert_eq!(
        kpis::remove(&alice.node, &org, &kpi).unwrap_err().kind,
        ErrorKind::NotFound
    );
}

#[tokio::test]
async fn setting_a_plan_again_keeps_its_aims() {
    let (alice, _) = alice().await;
    let (org, team, kpi) = (id("acme"), id("app"), id("revenue"));
    kpis::add(&alice.node, &org, revenue()).unwrap();
    plan_team(&alice.node, &org, &team, Some(plan())).unwrap();
    kpis::aim(&alice.node, &org, &team, "app#1".into(), &kpi).unwrap();

    let mut other = plan();
    other.weekly_budget_usd = 900;
    plan_team(&alice.node, &org, &team, Some(other)).unwrap();
    let kept = body(&alice).team(&team).unwrap().plan.clone().unwrap();
    assert_eq!(
        (kept.weekly_budget_usd, kept.aims.len()),
        (900, 1),
        "the aims stay"
    );

    plan_team(&alice.node, &org, &team, None).unwrap();
    assert!(
        body(&alice).team(&team).unwrap().plan.is_none(),
        "clearing the plan clears its aims"
    );
}

#[tokio::test]
async fn only_the_root_changes_what_the_org_measures() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let added = kpis::add(&bob.node, &id("acme"), revenue());
    assert_eq!(added.unwrap_err().kind, ErrorKind::Forbidden);
    assert!(body(&bob).kpis.is_empty());
}
