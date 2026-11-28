//! What an org measures and aims at, and a team's plan, as every node checks them.
use agency_proto::canonical::canonical_of;
use agency_proto::org::{verify_successor, Allocation, SignedOrg, TeamPlan};
use agency_proto::outcome::{Aim, Currency, Kpi, KpiUnit, EPIC, MAX_AIMS, MAX_KPIS};
use agency_proto::{AccountKey, Kind, OrgDefinition};
mod common;
use common::org_vectors::*;
use common::*;

#[test]
fn what_an_org_measures_is_left_out_of_a_version_that_states_nothing() {
    let key = AccountKey::from_seed([7u8; 32]);
    let canonical = canonical_of(&genesis(&key).body).unwrap();
    assert!(
        !canonical.contains("kpis") && !canonical.contains("aims"),
        "{canonical}"
    );
}

#[test]
fn a_kpi_is_written_with_its_unit_and_currency_beside_each_other() {
    let yen = kpi(
        "revenue",
        "Revenue",
        KpiUnit::Money {
            currency: Currency::parse("JPY").unwrap(),
        },
    );
    let written = serde_json::to_value(&yen).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "id": "revenue", "label": "Revenue", "kind": "business", "unit": "money", "currency": "JPY", "goodWhen": "higher" })
    );
    assert_eq!(serde_json::from_value::<Kpi>(written).unwrap(), yen);
    let seconds = serde_json::to_value(kpi("lead-time", "Lead time", KpiUnit::Seconds)).unwrap();
    assert_eq!(seconds["unit"], "seconds");
    assert!(seconds.get("currency").is_none());
    for bad in [
        serde_json::json!({ "id": "r", "label": "R", "kind": "business", "unit": "usd", "goodWhen": "higher" }),
        serde_json::json!({ "id": "r", "label": "R", "kind": "business", "unit": "money", "goodWhen": "higher" }),
        serde_json::json!({ "id": "r", "label": "R", "kind": "business", "unit": "money", "currency": "yen", "goodWhen": "higher" }),
    ] {
        assert!(serde_json::from_value::<Kpi>(bad.clone()).is_err(), "{bad}");
    }
    for bad in ["jpy", "JP", "JPYY", "J1Y", "", "円円円"] {
        assert!(Currency::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn what_an_org_measures_and_aims_at_is_checked_by_every_node() {
    let key = AccountKey::from_seed([7u8; 32]);
    let with = |kpis: Vec<Kpi>, aims: Vec<Aim>| {
        let mut body = genesis(&key).body;
        body.kpis = kpis;
        body.teams_mut()[0].plan = Some(TeamPlan::new(allocation(), 0).with_aims(aims));
        body.validate()
    };
    let count = || kpi("count", "Count", KpiUnit::Count);
    assert_eq!(with(vec![count()], vec![aim("app#1", "count")]), Ok(()));
    assert_eq!(with(vec![], vec![]), Ok(()));
    assert!(with(vec![count(), count()], vec![]).is_err(), "a KPI twice");
    assert!(
        with(vec![kpi("count", "", KpiUnit::Count)], vec![]).is_err(),
        "a label"
    );
    let many: Vec<Kpi> = (0..=MAX_KPIS)
        .map(|n| kpi(&format!("k{n}"), "K", KpiUnit::Count))
        .collect();
    assert!(with(many, vec![]).is_err(), "more than {MAX_KPIS} KPIs");
    assert!(
        with(vec![count()], vec![aim("app#1", "other")]).is_err(),
        "a KPI that is not stated"
    );
    assert!(
        with(
            vec![count()],
            vec![aim("app#1", "count"), aim("app#1", "count")]
        )
        .is_err(),
        "repeated"
    );
    assert_eq!(
        with(
            vec![count()],
            vec![aim("app#1", "count"), aim("app#2", "count")]
        ),
        Ok(()),
        "two epics may aim at one KPI"
    );
    for epic in ["", &"e".repeat(EPIC + 1), "app\n#1"] {
        assert!(
            with(vec![count()], vec![aim(epic, "count")]).is_err(),
            "{epic:?}"
        );
    }
    let aims: Vec<Aim> = (0..=MAX_AIMS)
        .map(|n| aim(&format!("app#{n}"), "count"))
        .collect();
    assert!(
        with(vec![count()], aims).is_err(),
        "more than {MAX_AIMS} aims"
    );
}

/// A change to a version's JSON, to make it as a sender with another idea of the shape would.
type Change = fn(&mut serde_json::Value);

/// What a receiving node does with a version the root signed: it reads the JSON, and checks the signature
/// over the canonical JSON of what it read (`Signed::verify`), not over the bytes it was sent.
fn received(
    org: &SignedOrg,
    root: &AccountKey,
    change: impl FnOnce(&mut serde_json::Value),
) -> Result<(), String> {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.kpis = vec![kpi("revenue", "Revenue", KpiUnit::Percent)];
    let mut value = serde_json::to_value(&body).unwrap();
    change(&mut value);
    // The root signs the bytes it holds, the unknown field included.
    let text = agency_proto::canonical::canonical(&value);
    let signature = root.sign(&Kind::Org.message(&text));
    let sent = serde_json::json!({ "body": value, "signer": "alice", "signature": signature });
    let next: SignedOrg = serde_json::from_value(sent).map_err(|error| error.to_string())?;
    verify_successor(org, &next).map(|_| ())
}

#[test]
fn a_kpi_with_a_field_this_node_does_not_know_is_refused_by_the_signature() {
    let key = AccountKey::from_seed([7u8; 32]);
    let org = genesis(&key);
    assert_eq!(
        received(&org, &key, |_| {}),
        Ok(()),
        "the same version without the extra field"
    );

    // `Kpi` cannot refuse unknown fields while parsing (its unit is flattened), so the parse drops them. The
    // signature is checked over what was parsed, which no longer holds the field the root signed.
    let changes: [(&str, Change); 2] = [
        ("an extra field on a KPI", |v| {
            v["kpis"][0]["target"] = serde_json::json!(5)
        }),
        ("a currency on a unit that has none", |v| {
            v["kpis"][0]["currency"] = serde_json::json!("JPY")
        }),
    ];
    for (what, change) in changes {
        let refused = received(&org, &key, change);
        assert_eq!(
            refused,
            Err("not signed by the current root key".to_owned()),
            "{what} is refused by the signature, not accepted"
        );
    }

    // The other definition structs refuse unknown fields while parsing.
    let aimed = |v: &mut serde_json::Value| {
        v["teams"][0]["plan"] = serde_json::json!({
            "allocation": { "newFeature": 100, "improvement": 0, "maintenance": 0, "debt": 0, "incident": 0 },
            "weeklyBudgetUsd": 0,
            "aims": [{ "epic": "app#1", "kpi": "revenue", "target": 5 }]
        });
    };
    let refused = received(&org, &key, aimed);
    assert!(
        refused.is_err(),
        "an extra field on an aim is refused: {refused:?}"
    );
}

#[test]
fn a_plan_allocation_adds_up_to_100() {
    let key = AccountKey::from_seed([7u8; 32]);
    let mut body = genesis(&key).body;
    let mut allocation = Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    };
    body.teams_mut()[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation: allocation.clone(),
        weekly_budget_usd: 0,
    });
    assert_eq!(body.validate(), Ok(()));
    allocation.incident = 4;
    body.teams_mut()[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation: allocation.clone(),
        weekly_budget_usd: 0,
    });
    assert!(body.validate().is_err(), "99");
    allocation.incident = u32::MAX;
    body.teams_mut()[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation,
        weekly_budget_usd: 0,
    });
    assert!(body.validate().is_err(), "no overflow to 100");
}
