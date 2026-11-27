//! The federation routes that change what the node keeps (a definition, a join, a device change) are open to anyone,
//! so none of them takes the node's write lock before the request has shown what it must: a request that no key
//! the node accepts signed, or a join without an open invite, is refused without a turn at the lock. Counted here
//! with a lock that tallies its takers.
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::memory::{CountingRandom, ManualClock, MemoryRuntime, NoNotifier};
use agency_adapters::peers::HttpPeers;
use agency_core::config::{Account, NodeConfig};
use agency_core::devices::accept_device_change;
use agency_core::invites::{accept_join, invite, InviteRecord};
use agency_core::orgs::{self, receive};
use agency_core::ports::{ConfigStore, WriteLock};
use agency_core::{ErrorKind, Node, Ports};
use agency_proto::device::{DeviceChange, DeviceOp};
use agency_proto::invite::{code_digest, Invite};
use agency_proto::org::{Root, SignedOrg};
use agency_proto::{
    AccountKey, Device, Id, JoinRequest, Kind, OrgDefinition, Person, Role, Signed, Timestamp,
};
use async_trait::async_trait;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

const T0: &str = "2026-11-24T12:00:00Z";

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

#[derive(Default)]
struct CountingLock {
    takers: AtomicUsize,
    inner: tokio::sync::Mutex<()>,
}

#[async_trait]
impl WriteLock for CountingLock {
    async fn lock(&self) -> Box<dyn Send + '_> {
        self.takers.fetch_add(1, Ordering::SeqCst);
        Box::new(self.inner.lock().await)
    }
}

struct Fixture {
    node: Node,
    lock: Arc<CountingLock>,
    _dir: TempDir,
}

impl Fixture {
    fn takers(&self) -> usize {
        self.lock.takers.load(Ordering::SeqCst)
    }
}

fn node_of(account: &str) -> Fixture {
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
    let lock = Arc::new(CountingLock::default());
    let ports = Ports {
        store: Arc::new(data),
        peers: Arc::new(HttpPeers::new().unwrap()),
        runtime: Arc::new(MemoryRuntime::default()),
        clock: Arc::new(ManualClock::at(T0)),
        random: Arc::new(CountingRandom::default()),
        lock: lock.clone(),
        funding: Arc::new(UsageFiles::new(dir.path().join("node"))),
        delivery: Arc::new(agency_adapters::memory::MemoryDelivery::default()),
        notifier: Arc::new(NoNotifier),
        journal: Arc::new(agency_adapters::memory::NoJournal),
        root_keys: Arc::new(agency_adapters::memory::MemoryRootKeys::default()),
    };
    Fixture {
        node: Node::open(ports, key).unwrap(),
        lock,
        _dir: dir,
    }
}

/// alice's node, the root of org acme, in which alice has one device.
fn alice() -> Fixture {
    let fixture = node_of("alice");
    orgs::create(&fixture.node, id("acme"), "Acme".into()).unwrap();
    let laptop = Device {
        id: laptop().public().device_id(),
        label: "Laptop".into(),
        public_key: laptop().public(),
        added_at: Timestamp::parse(T0).unwrap(),
    };
    orgs::amend(&fixture.node, &id("acme"), |body| {
        body.people[0].devices.push(laptop);
        Ok(())
    })
    .unwrap();
    fixture
}

fn laptop() -> AccountKey {
    AccountKey::from_seed([3; 32])
}

fn join_request(
    code: &str,
    signer: &AccountKey,
    adds: &AccountKey,
    at: &str,
) -> Signed<JoinRequest> {
    let body = JoinRequest {
        org: id("acme"),
        code: code.into(),
        account: id("bob"),
        label: "Bob".into(),
        public_key: adds.public(),
        at: at.into(),
    };
    Signed::sign(Kind::Join, body, id("bob"), signer).unwrap()
}

fn device_change(account: &str, signer: &AccountKey) -> Signed<DeviceChange> {
    let phone = AccountKey::from_seed([4; 32]).public();
    let body = DeviceChange {
        org: id("acme"),
        account: id(account),
        op: DeviceOp::Add {
            device: Box::new(Device {
                id: phone.device_id(),
                label: "Phone".into(),
                public_key: phone,
                added_at: Timestamp::parse(T0).unwrap(),
            }),
        },
        at: Timestamp::parse(T0).unwrap(),
    };
    Signed::sign(Kind::Device, body, signer.public().device_id(), signer).unwrap()
}

fn definition(root: &AccountKey, signer: &AccountKey) -> SignedOrg {
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
async fn a_definition_no_accepted_key_signed_is_refused_without_the_lock() {
    let carol = node_of("carol");
    let (root, mallory) = (AccountKey::generate(), AccountKey::generate());
    carol
        .node
        .update_config(|config| config.trust(id("acme"), root.public().fingerprint()))
        .unwrap();
    let error = receive(&carol.node, definition(&mallory, &mallory))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "a first copy of an untrusted root"
    );
    assert_eq!(carol.takers(), 0);

    receive(&carol.node, definition(&root, &root))
        .await
        .unwrap();
    assert_eq!(
        carol.takers(),
        1,
        "a first copy of the trusted root waits its turn"
    );

    let next = |signer: &AccountKey| {
        let current = carol.node.store().load_org(&id("acme")).unwrap();
        let mut body = OrgDefinition::successor_of(&current, T0.into());
        body.label = "Acme again".into();
        Signed::sign(Kind::Org, body, id("alice"), signer).unwrap()
    };
    let error = receive(&carol.node, next(&mallory)).await.unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "a next version of another key"
    );
    assert_eq!(carol.takers(), 1, "refused without a turn at the lock");

    receive(&carol.node, next(&root)).await.unwrap();
    assert_eq!(carol.takers(), 2, "the root's next version waits its turn");
}

#[tokio::test]
async fn a_device_change_no_key_of_the_org_signed_is_refused_without_the_lock() {
    let alice = alice();
    let mallory = AccountKey::generate();
    for (account, signer, what) in [
        ("alice", &mallory, "a key that is not the person's"),
        ("mallory", &mallory, "a stranger"),
    ] {
        let error = accept_device_change(&alice.node, &id("acme"), device_change(account, signer))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::NotFound, "{what}");
    }
    assert_eq!(alice.takers(), 0);

    accept_device_change(&alice.node, &id("acme"), device_change("alice", &laptop()))
        .await
        .unwrap();
    assert_eq!(
        alice.takers(),
        1,
        "a change by the person's device waits its turn"
    );
}

#[tokio::test]
async fn a_join_that_is_forged_stale_or_without_an_open_invite_is_refused_without_the_lock() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    let (bob, mallory) = (AccountKey::generate(), AccountKey::generate());
    let cases = [
        (
            join_request(&code, &mallory, &bob, T0),
            ErrorKind::Unauthorized,
            "signed by another key",
        ),
        (
            join_request(&code, &bob, &bob, "2026-11-24T12:10:01Z"),
            ErrorKind::BadRequest,
            "from the future",
        ),
        (
            join_request("no-such-code", &bob, &bob, T0),
            ErrorKind::Forbidden,
            "no invite of that code",
        ),
    ];
    for (request, kind, what) in cases {
        let error = accept_join(&alice.node, &id("acme"), request)
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind, "{what}");
    }
    assert_eq!(alice.takers(), 0);

    accept_join(
        &alice.node,
        &id("acme"),
        join_request(&code, &bob, &bob, T0),
    )
    .await
    .unwrap();
    assert_eq!(
        alice.takers(),
        1,
        "a join with an open invite waits its turn"
    );
}

/// Runs `request` while the test holds the node's write lock, waits until it is queued behind it (it has made
/// the checks it makes before the lock), lets `change` alter what the node keeps, and then lets the lock go.
/// Returns what the request answered once it got its turn: the state it checked before the lock is gone by then.
async fn answered_after_a_change<T, F>(
    fixture: &Arc<Fixture>,
    request: F,
    change: impl FnOnce(&Fixture),
) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let guard = fixture.lock.lock().await;
    let held = fixture.takers();
    let task = tokio::spawn(request);
    tokio::time::timeout(Duration::from_secs(10), async {
        while fixture.takers() == held {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the request queues for the lock");
    change(fixture);
    drop(guard);
    task.await.unwrap()
}

#[tokio::test]
async fn a_join_whose_invite_was_used_while_it_waited_for_the_lock_is_refused() {
    let alice = Arc::new(alice());
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    let bob = AccountKey::generate();
    let request = join_request(&code, &bob, &bob, T0);
    let queued = alice.clone();
    let answer = answered_after_a_change(
        &alice,
        async move { accept_join(&queued.node, &id("acme"), request).await },
        |fixture| {
            let digest = code_digest(&code);
            let record = fixture.node.store().load_invite(&digest).unwrap();
            let used = InviteRecord {
                used_by: Some(id("carol")),
                ..record
            };
            fixture.node.store().save_invite(&digest, &used).unwrap();
        },
    )
    .await;
    assert_eq!(
        answer.unwrap_err().kind,
        ErrorKind::Forbidden,
        "the invite was open when the join arrived and used when it got its turn"
    );
    let org = alice.node.store().load_org(&id("acme")).unwrap();
    assert!(org.body.member(&id("bob")).is_none(), "bob did not get in");
}

#[tokio::test]
async fn a_device_change_whose_key_was_removed_while_it_waited_for_the_lock_is_refused() {
    let alice = Arc::new(alice());
    let change = device_change("alice", &laptop());
    let queued = alice.clone();
    let answer = answered_after_a_change(
        &alice,
        async move { accept_device_change(&queued.node, &id("acme"), change).await },
        |fixture| {
            orgs::amend(&fixture.node, &id("acme"), |body| {
                body.people[0].devices.clear();
                body.revoked.push(laptop().public());
                Ok(())
            })
            .unwrap();
        },
    )
    .await;
    assert_eq!(
        answer.unwrap_err().kind,
        ErrorKind::NotFound,
        "the key signed for a device the org had when the change arrived and no longer has"
    );
    let org = alice.node.store().load_org(&id("acme")).unwrap();
    assert!(org.body.people[0].devices.is_empty(), "no device was added");
}

#[tokio::test]
async fn a_definition_checked_against_a_root_that_changed_while_it_waited_is_refused() {
    let carol = Arc::new(node_of("carol"));
    let (root, other) = (AccountKey::generate(), AccountKey::generate());
    carol
        .node
        .update_config(|config| config.trust(id("acme"), root.public().fingerprint()))
        .unwrap();
    receive(&carol.node, definition(&root, &root))
        .await
        .unwrap();
    let current = carol.node.store().load_org(&id("acme")).unwrap();
    let mut body = OrgDefinition::successor_of(&current, T0.into());
    body.label = "Acme again".into();
    let next = Signed::sign(Kind::Org, body, id("alice"), &root).unwrap();

    let queued = carol.clone();
    let answer = answered_after_a_change(
        &carol,
        async move { receive(&queued.node, next).await },
        |fixture| {
            fixture
                .node
                .store()
                .save_org(&definition(&other, &other))
                .unwrap();
        },
    )
    .await;
    assert_eq!(
        answer.unwrap_err().kind,
        ErrorKind::NotFound,
        "signed by the root the copy had when it arrived, and not the one it has when it gets its turn"
    );
    let kept = carol.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(kept.body.label, "Acme", "the version was not taken");
}
