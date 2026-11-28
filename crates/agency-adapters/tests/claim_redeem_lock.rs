//! A claim link is spent once, whatever else is waiting for the node's write lock: a redeem checks the link
//! before the lock, and again under it, because another redeem may have spent the link while it waited.
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::memory::{CountingRandom, ManualClock, MemoryRuntime, NoNotifier};
use agency_adapters::peers::HttpPeers;
use agency_core::claims::{self, ClaimState, Redemption};
use agency_core::config::{Account, NodeConfig};
use agency_core::ports::{ConfigStore, WriteLock};
use agency_core::{orgs, seats, ErrorKind, Node, Ports};
use agency_proto::seat::Seat;
use agency_proto::{AccountKey, Id};
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

fn carol_seat() -> Seat {
    Seat {
        id: id("carol"),
        label: "Carol (finance)".into(),
        department: None,
        unit: None,
        ask_about: vec![],
        relays: vec![id("alice")],
    }
}

#[tokio::test]
async fn a_link_spent_while_a_redeem_waited_for_the_lock_is_not_spent_again() {
    let alice = Arc::new(node_of("alice"));
    orgs::create(&alice.node, id("acme"), "Acme".into()).unwrap();
    seats::add_seat(&alice.node, &id("acme"), carol_seat()).unwrap();
    let link =
        claims::seat_invite(&alice.node, &id("acme"), id("carol"), "carol@example.com").unwrap();
    let code = link.link.split("code=").nth(1).unwrap().to_owned();
    let digest = agency_proto::invite::code_digest(&code);
    let (first, second) = (
        AccountKey::from_seed([0x71; 32]),
        AccountKey::from_seed([0x72; 32]),
    );

    let queued = alice.clone();
    let key = second.public().encoded();
    let answer =
        answered_after_a_change(
            &alice,
            {
                let code = code.clone();
                async move {
                    claims::redeem(&queued.node, &code, "carol@example.com", &key, "Phone").await
                }
            },
            |fixture| {
                let mut record = fixture.node.store().load_claim(&digest).unwrap();
                record.state = ClaimState::Redeemed(Redemption {
                    public_key: first.public(),
                    label: "Laptop".into(),
                    redeemed_at: fixture.node.now_text(),
                });
                fixture.node.store().save_claim(&digest, &record).unwrap();
            },
        )
        .await;

    assert_eq!(
        answer.unwrap_err().kind,
        ErrorKind::NotFound,
        "the link was open when the redeem arrived and spent when it got its turn"
    );
    let record = alice
        .node
        .store()
        .load_claim(&agency_proto::invite::code_digest(&code))
        .unwrap();
    let ClaimState::Redeemed(kept) = record.state else {
        panic!("{record:?}")
    };
    assert_eq!(
        kept.public_key,
        AccountKey::from_seed([0x71; 32]).public(),
        "the first key is the one kept"
    );
}
