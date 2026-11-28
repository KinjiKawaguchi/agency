//! Builders of the vectors of org definitions: a root's rotation and recovery, a plan, seats, a hosted member and
//! what an org measures.
#![allow(dead_code)]
use super::*;

/// Test vectors: canonical JSON and signatures of fixed bodies with keys from fixed seeds: the account key (seed 7) signs
/// the org definition and the first device change, the device key (seed 9) signs the envelope and a comment, and a
/// P-256 device key signs the same comment and two device changes. Another implementation
/// must produce the same values.
/// An org moved to a new root key (seed `0d…`): version 2 names it and is signed by the old root, and
/// version 3 is signed by the new one.
pub fn root_rotation(old: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let new = AccountKey::from_seed([0x0d; 32]);
    let mut naming = OrgDefinition::successor_of(org, AT.into());
    naming.root.public_key = new.public();
    let naming = Signed::sign(Kind::Org, naming, id("alice"), old).unwrap();
    assert!(verify_successor(org, &naming).is_ok());
    let mut after = OrgDefinition::successor_of(&naming, AT.into());
    after.label = "Acme Inc".into();
    let after = Signed::sign(Kind::Org, after, id("alice"), &new).unwrap();
    assert!(verify_successor(&naming, &after).is_ok());
    let by_old = Signed::sign(Kind::Org, after.body.clone(), id("alice"), old).unwrap();
    assert!(
        verify_successor(&naming, &by_old).is_err(),
        "the old root signs no later version"
    );
    let signed = |doc: &SignedOrg| serde_json::json!({ "canonical": canonical_of(&doc.body).unwrap(), "signature": doc.signature, "digest": doc.digest() });
    serde_json::json!({
        "newSeed": "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d",
        "newPublicKey": new.public().encoded(),
        "namingTheNewRoot": signed(&naming),
        "signedByTheNewRoot": signed(&after),
    })
}

/// An org given a recovery key (seed `0e…`), then recovered to a new root key (seed `0f…`): the root adds
/// the recovery key, the recovery key starts the recovery and, after the wait, finishes it; the root may
/// instead cancel the start by signing any version. Then the recovery key replaces itself (seed `10…`)
/// the same way. The refused versions show what neither key may sign.
pub fn root_recovery(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let (recovery, new_root, new_recovery) = (
        AccountKey::from_seed([0x0e; 32]),
        AccountKey::from_seed([0x0f; 32]),
        AccountKey::from_seed([0x10; 32]),
    );
    let sign = |body: OrgDefinition, key: &AccountKey| {
        Signed::sign(Kind::Org, body, id("alice"), key).unwrap()
    };
    let next = |doc: &SignedOrg| OrgDefinition::successor_of(doc, AT.into());
    let is = |current: &SignedOrg, doc: &SignedOrg, succession: Succession| {
        assert_eq!(verify_successor(current, doc), Ok(succession));
    };

    let mut body = next(org);
    body.root.recovery = Some(recovery.public());
    let with_recovery = sign(body, root);
    is(org, &with_recovery, Succession::Root);

    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: Some(new_root.public()),
        recovery: None,
    });
    let start = sign(body, &recovery);
    is(&with_recovery, &start, Succession::RecoveryStart);

    let mut body = next(&start);
    body.root.pending = None;
    let cancel = sign(body, root);
    is(&start, &cancel, Succession::Root);

    let mut body = next(&start);
    body.root.public_key = new_root.public();
    body.root.pending = None;
    let finish = sign(body, &recovery);
    is(&start, &finish, Succession::RecoveryFinish);
    assert!(
        verify_root_successor(&start, &finish).is_err(),
        "an answer from the root's node is never a recovery"
    );
    assert!(verify_root_successor(&start, &cancel).is_ok());
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: None,
        recovery: None,
    });
    let empty_start = sign(body, &recovery);
    for kept_on_a_node in [
        root.public(),
        org.body.people[0].public_key.clone().unwrap(),
    ] {
        let mut body = next(org);
        body.root.recovery = Some(kept_on_a_node);
        assert!(
            body.validate().unwrap_err().contains("recovery key"),
            "a recovery key a node keeps"
        );
    }
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: None,
        recovery: Some(root.public()),
    });
    assert!(
        body.validate().is_err(),
        "a new recovery key that is the root key"
    );
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: org.body.people[0].public_key.clone(),
        recovery: None,
    });
    assert!(
        body.validate().is_err(),
        "a new root key that is an account key"
    );
    assert!(verify_successor(&with_recovery, &empty_start)
        .unwrap_err()
        .contains("names no change"));

    let mut body = next(&finish);
    body.root.pending = Some(PendingRoot {
        public_key: None,
        recovery: Some(new_recovery.public()),
    });
    let replace_start = sign(body, &recovery);
    is(&finish, &replace_start, Succession::RecoveryStart);
    let mut body = next(&replace_start);
    body.root.recovery = Some(new_recovery.public());
    body.root.pending = None;
    let replace_finish = sign(body, &recovery);
    is(&replace_start, &replace_finish, Succession::RecoveryFinish);

    let refused = |current: &SignedOrg, doc: SignedOrg, why: &str| {
        let error = verify_successor(current, &doc).unwrap_err();
        assert!(error.contains(why), "{error}");
        serde_json::json!({ "after": current.body.version, "canonical": canonical_of(&doc.body).unwrap(), "signature": doc.signature, "why": error })
    };
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: Some(new_root.public()),
        recovery: None,
    });
    body.label = "Taken".into();
    let start_changing_more = refused(
        &with_recovery,
        sign(body, &recovery),
        "changes nothing but the root",
    );
    let mut body = next(&with_recovery);
    body.root.public_key = new_root.public();
    let rotation_without_a_start =
        refused(&with_recovery, sign(body, &recovery), "names no change");
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: Some(new_root.public()),
        recovery: None,
    });
    let start_by_root = refused(
        &with_recovery,
        sign(body, root),
        "does not start a recovery",
    );
    let mut body = next(&with_recovery);
    body.root.recovery = Some(new_recovery.public());
    let recovery_by_root = refused(&with_recovery, sign(body, root), "only the recovery key");

    let signed = |doc: &SignedOrg| serde_json::json!({ "canonical": canonical_of(&doc.body).unwrap(), "signature": doc.signature, "digest": doc.digest() });
    serde_json::json!({
        "recoverySeed": "0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e",
        "newRootSeed": "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f",
        "newRecoverySeed": "1010101010101010101010101010101010101010101010101010101010101010",
        "addingTheRecoveryKey": signed(&with_recovery),
        "start": signed(&start),
        "cancel": signed(&cancel),
        "finish": signed(&finish),
        "replacingTheWordsStart": signed(&replace_start),
        "replacingTheWordsFinish": signed(&replace_finish),
        "refused": {
            "startChangingMore": start_changing_more,
            "rotationWithoutAStart": rotation_without_a_start,
            "startSignedByTheRoot": start_by_root,
            "recoveryChangedByTheRoot": recovery_by_root,
        },
    })
}

/// The next version of `org`, with a plan for its team: how a plan is signed.
pub fn org_with_plan(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.teams[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation: Allocation {
            new_feature: 40,
            improvement: 20,
            maintenance: 20,
            debt: 15,
            incident: 5,
        },
        weekly_budget_usd: 500,
    });
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

/// The next version of `org`, with a seat and an external party: how they are signed. The seat has no account,
/// key or device; both are answered for by the member `owner`.
pub fn org_with_seats(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.seats.push(Seat {
        id: id("carol"),
        label: "Carol (finance)".into(),
        department: Some(id("dev")),
        ask_about: vec!["invoices".into(), "budget approval".into()],
        relays: vec![id("owner")],
    });
    body.externals.push(External {
        id: id("erin"),
        label: "Erin (outside counsel)".into(),
        ask_about: vec!["contract review".into(), "legal checks".into()],
        relays: vec![id("owner")],
    });
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

/// The next version of `org`, with a member hosted by the root's node: no account key (`publicKey` left out),
/// `hostedBy` "root", its member id as its account, and one device, a P-256 key from a browser.
pub fn org_with_hosted_member(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    let browser = AccountKey::from_seed([0x61; 32]).public();
    let device = Device {
        id: browser.device_id(),
        label: "Carol's laptop".into(),
        public_key: browser,
        added_at: Timestamp::parse(AT).unwrap(),
    };
    body.people
        .push(Person::hosted(id("carol"), "Carol", vec![device]));
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    assert!(!canonical_of(&signed.body)
        .unwrap()
        .contains("\"publicKey\":null"));
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

/// The next version of `org`, in which dan has taken the seat carol: the seat is gone from `seats`, and dan has
/// its id as `formerly` and what to ask about it as `askAbout`.
pub fn org_with_a_claimed_seat(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.people.push(
        Person::new(
            id("dan"),
            "Dan",
            id("dan"),
            AccountKey::from_seed([4; 32]).public(),
        )
        .with_department(Some(id("dev")))
        .with_formerly(vec![id("carol")])
        .with_ask_about(vec!["invoices".into(), "budget approval".into()]),
    );
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

pub fn allocation() -> Allocation {
    Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    }
}

pub fn kpi(raw: &str, label: &str, unit: KpiUnit) -> Kpi {
    Kpi {
        id: id(raw),
        label: label.into(),
        kind: KpiKind::Business,
        unit,
        good_when: GoodWhen::Higher,
    }
}

pub fn aim(epic: &str, kpi: &str) -> Aim {
    Aim {
        epic: epic.into(),
        kpi: id(kpi),
    }
}

/// The next version of `org`, with KPIs and a team plan that aims at one: how what an org measures is signed.
pub fn org_with_outcomes(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    let yen = KpiUnit::Money {
        currency: Currency::parse("JPY").unwrap(),
    };
    let mut activation = kpi("activation", "Activation", KpiUnit::Percent);
    activation.kind = KpiKind::Product;
    let mut lead_time = kpi("lead-time", "Lead time of a feature", KpiUnit::Seconds);
    lead_time.kind = KpiKind::Product;
    lead_time.good_when = GoodWhen::Lower;
    body.kpis = vec![kpi("revenue", "Revenue", yen), activation, lead_time];
    let plan = TeamPlan::new(allocation(), 500).with_aims(vec![aim("app#10", "activation")]);
    body.teams[0].plan = Some(plan);
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}
