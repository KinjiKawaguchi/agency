//! Use cases against real files and a manual clock: expiry and timing (the envelope's 7 days, the invite's
//! 7 days, the signed read's 5 minutes, the join request's 10 minutes, the outbox backoff) and the
//! checks that decide whether a signed document is accepted.
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::memory::{CountingRandom, ManualClock, MemoryRuntime, NoNotifier};
use agency_adapters::peers::HttpPeers;
use agency_adapters::system::TokioWriteLock;
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::decisions::{accept_envelope, Accepted, Decided};
use agency_core::devices::DeviceSigner;
use agency_core::invites::{accept_join, invite, open_invite};
use agency_core::orgs::{self, receive};
use agency_core::outbox::{deliver_due, pending, Parcel, ParcelBody};
use agency_core::ports::ConfigStore;
use agency_core::read_auth::{sign_read, verify_read, ReadPolicy};
use agency_core::signed_decisions::decide_with_device;
use agency_core::{ErrorKind, Node, Ports};
use agency_proto::clock::Duration;
use agency_proto::invite::Invite;
use agency_proto::org::{Root, SignedOrg, TeamPlace};
use agency_proto::{
    AccountKey, Device, Envelope, EnvelopeId, EnvelopeKind, Id, JoinRequest, Kind, OrgDefinition,
    Payload, Person, Role, Signed, Timestamp,
};
use std::sync::Arc;
use tempfile::TempDir;

const T0: &str = "2026-11-24T12:00:00Z";
/// Nothing listens here, so every delivery is unreachable.
const NOWHERE: &str = "http://127.0.0.1:1";

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

struct Fixture {
    node: Node,
    clock: ManualClock,
    runtime: MemoryRuntime,
    _dir: TempDir,
}

/// alice's node at T0: the root of org acme with one device, hosting team app with one waiting request r-1.
fn alice() -> Fixture {
    let fixture = bare("alice");
    let node = &fixture.node;
    orgs::create(node, id("acme"), "Acme".into()).unwrap();
    // The team is in the org's definition, as `team-add` makes it: a node serves only such a team.
    let place = TeamPlace::new(id("app"), "app", node.url());
    orgs::add_team(node, &id("acme"), place).unwrap();
    let runtime = RuntimeCommand::of(vec!["unused".into()]);
    let hosted = HostedTeam::new(id("acme"), id("app"), id("alice"), runtime);
    node.update_config(|config| config.teams.push(hosted))
        .unwrap();
    let laptop = Device {
        id: laptop().public().device_id(),
        label: "Laptop".into(),
        public_key: laptop().public(),
        added_at: Timestamp::parse(T0).unwrap(),
    };
    orgs::amend(node, &id("acme"), |body| {
        body.people[0].devices.push(laptop);
        Ok(())
    })
    .unwrap();
    fixture
}

/// alice's device in the fixture; only it signs her decisions.
fn laptop() -> AccountKey {
    AccountKey::from_seed([3; 32])
}

/// A node at T0 with an account and nothing else.
fn bare(account: &str) -> Fixture {
    let dir = TempDir::new().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    let key = data.create_key().unwrap();
    let account = Account {
        id: id(account),
        label: account.to_uppercase(),
    };
    let url = format!("http://{}.example", account.id);
    data.save_config(&NodeConfig::new(url, "127.0.0.1:0", account, "scrum"))
        .unwrap();
    let clock = ManualClock::at(T0);
    let runtime = MemoryRuntime::default();
    runtime
        .0
        .lock()
        .unwrap()
        .requests
        .push(("r-1".into(), "Make a key?".into()));
    let ports = Ports {
        store: Arc::new(data),
        peers: Arc::new(HttpPeers::new().unwrap()),
        runtime: Arc::new(runtime.clone()),
        clock: Arc::new(clock.clone()),
        random: Arc::new(CountingRandom::default()),
        lock: Arc::new(TokioWriteLock::default()),
        funding: Arc::new(UsageFiles::new(dir.path().join("node"))),
        delivery: Arc::new(agency_adapters::memory::MemoryDelivery::default()),
        notifier: Arc::new(NoNotifier),
        journal: Arc::new(agency_adapters::memory::NoJournal),
        root_keys: Arc::new(agency_adapters::memory::MemoryRootKeys::default()),
    };
    let node = Node::open(ports, key).unwrap();
    Fixture {
        node,
        clock,
        runtime,
        _dir: dir,
    }
}

fn envelope(at: &str, byte: u8) -> Signed<Envelope> {
    let payload = Payload {
        text: String::new(),
        choice: Some("a".into()),
    };
    let body = Envelope {
        id: EnvelopeId::from_bytes([byte; 16]),
        kind: EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: "r-1".into(),
        payload,
        account: id("alice"),
        at: at.into(),
    };
    Signed::sign(
        Kind::Envelope,
        body,
        laptop().public().device_id(),
        &laptop(),
    )
    .unwrap()
}

#[tokio::test]
async fn envelopes_are_accepted_for_seven_days_and_up_to_five_minutes_ahead() {
    let alice = alice();
    let (org, team) = (id("acme"), id("app"));
    alice.clock.set("2026-12-01T12:00:00Z");
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1)).await,
        Ok(Accepted::Applied),
        "exactly 7 days old"
    );
    alice.clock.advance(Duration::seconds(1));
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 2))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest,
        "7 days and 1 second old"
    );

    alice.clock.set(T0);
    assert!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope("2026-11-24T12:05:00Z", 3)
        )
        .await
        .is_ok(),
        "5 minutes ahead"
    );
    assert_eq!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope("2026-11-24T12:05:01Z", 4)
        )
        .await
        .unwrap_err()
        .kind,
        ErrorKind::BadRequest,
        "more than 5 minutes ahead"
    );
    assert_eq!(alice.runtime.0.lock().unwrap().applied.len(), 2);
}

#[tokio::test]
async fn invites_open_for_seven_days() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    alice
        .clock
        .advance(Duration::days(7) - Duration::seconds(1));
    assert!(open_invite(&alice.node, &id("acme"), &code).is_some());
    alice.clock.advance(Duration::seconds(1));
    assert!(open_invite(&alice.node, &id("acme"), &code).is_none());
}

#[tokio::test]
async fn join_requests_must_be_signed_within_ten_minutes() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    let bob = AccountKey::generate();
    let request = |at: &str| {
        let body = JoinRequest {
            org: id("acme"),
            code: code.clone(),
            account: id("bob"),
            label: "Bob".into(),
            public_key: bob.public(),
            at: at.into(),
        };
        Signed::sign(Kind::Join, body, id("bob"), &bob).unwrap()
    };
    alice.clock.set("2026-11-24T12:10:01Z");
    assert_eq!(
        accept_join(&alice.node, &id("acme"), request(T0))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest
    );
    alice.clock.set("2026-11-24T12:10:00Z");
    let answer = accept_join(&alice.node, &id("acme"), request(T0))
        .await
        .unwrap();
    let agency_proto::root_change::RootAnswer::Signed(doc) = answer else {
        panic!("held: {answer:?}")
    };
    assert!(doc.body.member(&id("bob")).is_some());
}

#[test]
fn signed_reads_are_accepted_within_five_minutes_either_way() {
    let alice = alice();
    let definition = alice.node.store().load_org(&id("acme")).unwrap().body;
    let path = "/fed/v1/teams/acme/app/view";
    let signed_at = |offset: i64| {
        sign_read(
            alice.node.key(),
            &id("alice"),
            "GET",
            path,
            alice.node.now() + Duration::seconds(offset),
        )
    };
    let now = alice.node.now();
    for offset in [-300, 300] {
        assert!(
            verify_read(
                Some(&definition),
                true,
                &signed_at(offset),
                "GET",
                path,
                now,
                &ReadPolicy::open(None)
            )
            .is_ok(),
            "{offset}"
        );
    }
    for offset in [-301, 301] {
        assert_eq!(
            verify_read(
                Some(&definition),
                true,
                &signed_at(offset),
                "GET",
                path,
                now,
                &ReadPolicy::open(None)
            )
            .unwrap_err()
            .kind,
            ErrorKind::NotFound,
            "{offset}: refused like an unknown org"
        );
    }
    assert!(
        verify_read(
            Some(&definition),
            true,
            &signed_at(0),
            "GET",
            "/fed/v1/teams/acme/other/view",
            now,
            &ReadPolicy::open(None)
        )
        .is_err(),
        "the signature covers the path"
    );
}

#[tokio::test]
async fn the_outbox_retries_only_when_the_backoff_is_due() {
    let alice = alice();
    let place = TeamPlace::new(id("far"), "Far", NOWHERE);
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();
    let (key, key_id) = (laptop(), laptop().public().device_id());
    let device = DeviceSigner {
        id: &key_id,
        key: &key,
    };
    let outcome = decide_with_device(
        &alice.node,
        device,
        &id("acme"),
        &id("far"),
        EnvelopeKind::Answer,
        "r-9".into(),
        Payload {
            text: "yes".into(),
            choice: None,
        },
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Decided::Queued(_)), "{outcome:?}");
    let envelope = |parcels: &[Parcel]| {
        parcels
            .iter()
            .find(|one| matches!(one.body, ParcelBody::Envelope(_)))
            .cloned()
    };
    assert_eq!(envelope(&pending(&alice.node)).unwrap().attempts, 1);

    alice.clock.advance(Duration::seconds(14));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        envelope(&pending(&alice.node)).unwrap().attempts,
        1,
        "not due 14 seconds after the first attempt"
    );
    alice.clock.advance(Duration::seconds(1));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        envelope(&pending(&alice.node)).unwrap().attempts,
        2,
        "due after 15 seconds"
    );
    alice.clock.advance(Duration::seconds(29));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        envelope(&pending(&alice.node)).unwrap().attempts,
        2,
        "the second wait is 30 seconds"
    );
}

#[tokio::test]
async fn envelopes_must_be_signed_by_a_device_of_their_account_for_this_team() {
    let alice = alice();
    let (org, team) = (id("acme"), id("app"));
    let mut wrong_signer = envelope(T0, 1);
    wrong_signer.signer = id("bob");
    let error = accept_envelope(&alice.node, &org, &team, wrong_signer)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the signer must be one of the account's devices; refused like an unknown team"
    );
    let by_account = Signed::sign(
        Kind::Envelope,
        envelope(T0, 1).body,
        id("alice"),
        alice.node.key(),
    )
    .unwrap();
    let error = accept_envelope(&alice.node, &org, &team, by_account)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the account key, which the node holds, never signs a decision"
    );

    let error = accept_envelope(&alice.node, &org, &id("other"), envelope(T0, 2))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::BadRequest,
        "an envelope for acme/app sent to acme/other"
    );
    assert!(alice.runtime.0.lock().unwrap().applied.is_empty());
}

#[tokio::test]
async fn join_requests_must_be_signed_by_the_key_they_add() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    let (bob, mallory) = (AccountKey::generate(), AccountKey::generate());
    let body = JoinRequest {
        org: id("acme"),
        code: code.clone(),
        account: id("bob"),
        label: "Bob".into(),
        public_key: bob.public(),
        at: T0.into(),
    };
    let forged = Signed::sign(Kind::Join, body, id("bob"), &mallory).unwrap();
    let error = accept_join(&alice.node, &id("acme"), forged)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unauthorized);
    assert!(
        open_invite(&alice.node, &id("acme"), &code).is_some(),
        "the invite stays unused"
    );
}

/// A first copy of acme, signed by `signer` but naming `root` as its root key.
fn first_copy(root: &AccountKey, signer: &AccountKey) -> SignedOrg {
    let alice = Person::new(id("alice"), "Alice", id("alice"), root.public());
    let body = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        Root::new(id("alice"), root.public()),
        T0.into(),
    )
    .with_people(vec![alice]);
    Signed::sign(Kind::Org, body, id("alice"), signer).unwrap()
}

#[tokio::test]
async fn a_first_copy_must_be_signed_by_the_trusted_root_key() {
    let carol = bare("carol");
    let (root, mallory) = (AccountKey::generate(), AccountKey::generate());
    carol
        .node
        .update_config(|config| config.trust(id("acme"), root.public().fingerprint()))
        .unwrap();

    let error = receive(&carol.node, first_copy(&mallory, &mallory))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "a different root key than the trusted fingerprint; refused like an untrusted org"
    );
    let error = receive(&carol.node, first_copy(&root, &mallory))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the trusted root key, but signed by another key"
    );
    assert!(carol.node.store().load_org(&id("acme")).is_none());

    receive(&carol.node, first_copy(&root, &root))
        .await
        .unwrap();
    assert!(carol.node.store().load_org(&id("acme")).is_some());
}

#[tokio::test]
async fn an_applied_id_is_kept_while_its_envelope_can_still_be_sent_and_dropped_with_the_next_write_after(
) {
    let alice = alice();
    let (org, team) = (id("acme"), id("app"));
    let known = |byte: u8| {
        alice
            .node
            .store()
            .is_applied(&EnvelopeId::from_bytes([byte; 16]))
    };
    let applied = || alice.runtime.0.lock().unwrap().applied.len();
    let at = |plus: Duration| {
        let when = agency_proto::clock::parse(T0).unwrap() + plus;
        when.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };

    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1)).await,
        Ok(Accepted::Applied)
    );
    // Six days on: a copy within the envelope's life is still answered as a duplicate, and nothing is dropped.
    alice.clock.advance(Duration::days(6));
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1)).await,
        Ok(Accepted::Duplicate)
    );
    let now = at(Duration::days(6));
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(&now, 2)).await,
        Ok(Accepted::Applied)
    );
    assert!(known(1) && known(2));

    // Seven days and two minutes: the first envelope has expired, but its id is kept for five minutes more
    // (the skew an envelope's time may be ahead by), and a write now does not drop it.
    alice
        .clock
        .set(&at(Duration::days(7) + Duration::minutes(2)));
    assert_eq!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope(&alice.node.now_text(), 3)
        )
        .await,
        Ok(Accepted::Applied)
    );
    assert!(known(1), "within the window of 7 days and 5 minutes");

    // Past it, the next write drops the id, and a copy of the envelope is refused as expired: never applied again.
    alice
        .clock
        .set(&at(Duration::days(7) + Duration::minutes(6)));
    assert_eq!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope(&alice.node.now_text(), 4)
        )
        .await,
        Ok(Accepted::Applied)
    );
    assert!(
        !known(1),
        "older than the window: dropped as a new id was written"
    );
    assert!(
        known(2) && known(3) && known(4),
        "what is within the window stays"
    );
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest
    );
    assert_eq!(applied(), 4, "each envelope was applied once");
}
