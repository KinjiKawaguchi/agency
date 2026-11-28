//! Nodes for the integration tests: each in its own data directory, on its own port, with a manual clock
//! at T0 and an in-memory team runtime.
#![allow(dead_code)]
pub mod access;
pub mod guests;
pub mod oidc_issuer;

use agency_adapters::access::CertSource;
use agency_adapters::client_api::Assets;
use agency_adapters::files::DataDir;
use agency_adapters::launch::Launch;
use agency_adapters::memory::{
    CountingRandom, ManualClock, MemoryDelivery, MemoryNotifier, MemoryRootKeys, MemoryRuntime,
    MemoryRuntimeState,
};
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::invites::{self, join};
use agency_core::ports::{ConfigStore, TeamRuntime};
use agency_core::{orgs, Node};
use agency_node::{Opened, Overrides};
use agency_proto::org::TeamPlace;
use agency_proto::{Id, Payload, Role};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// The write token every test node embeds in its page.
pub const TOKEN: &str = "test-token-0123456789abcdef";

/// Web client files for tests: an index page and one script.
pub struct TestWeb;

impl Assets for TestWeb {
    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        let files: HashMap<&str, &[u8]> = HashMap::from([
            (
                "index.html",
                b"<html><head><meta name=\"agency-token\" content=\"__AGENCY_TOKEN__\" /><title>Agency</title></head><body></body></html>" as &[u8],
            ),
            ("assets/app.js", b"console.log(1)"),
        ]);
        files.get(path).map(|bytes| Cow::Borrowed(*bytes))
    }
}

/// Every node in these tests reads this time; nothing depends on the wall clock.
pub const T0: &str = "2026-11-24T12:00:00Z";

pub fn memory_runtime() -> (Arc<dyn TeamRuntime>, Arc<Mutex<MemoryRuntimeState>>) {
    let runtime = MemoryRuntime::default();
    let state = runtime.0.clone();
    (Arc::new(runtime), state)
}

pub fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

pub struct Running {
    pub node: Arc<Node>,
    pub clock: ManualClock,
    /// Records the notifications the node sends; it sends none until a token is registered.
    pub notifier: MemoryNotifier,
    /// The delivery facts the node reads; none until a test sets them.
    pub delivery: MemoryDelivery,
    /// The org root keys the node holds; a test can make it refuse to sign.
    pub root_keys: Arc<MemoryRootKeys>,
    opened: Opened,
    pub port: u16,
    /// The socket the node will serve on, held from the moment its port was chosen until `start`: a port
    /// that is chosen and then closed can be handed to another test before this node opens it again.
    listener: Option<TcpListener>,
    server: Option<JoinHandle<()>>,
    _dir: TempDir,
}

impl Running {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The node's log, for work a test starts as the running node would.
    pub fn logger(&self) -> agency_adapters::logging::Logger {
        self.opened.logger().clone()
    }

    /// The node's data directory.
    pub fn data_dir(&self) -> std::path::PathBuf {
        self._dir.path().join("node")
    }

    /// The node's HTTP app, to call without a socket (with a chosen peer address).
    pub fn app(&self) -> axum::Router {
        self.opened
            .app_with(self.port, TOKEN.into(), Arc::new(TestWeb))
            .unwrap()
    }

    pub async fn start(&mut self) {
        let listener = match self.listener.take() {
            Some(listener) => listener,
            None => TcpListener::bind(("127.0.0.1", self.port)).await.unwrap(),
        };
        let app = self
            .opened
            .app_with(self.port, TOKEN.into(), Arc::new(TestWeb))
            .unwrap();
        let service = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
        self.server = Some(tokio::spawn(async move {
            axum::serve(listener, service).await.unwrap()
        }));
    }

    pub async fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
            let _ = server.await;
        }
    }
}

pub async fn node(account: &str, runtime: Arc<dyn TeamRuntime>) -> Running {
    node_with(account, runtime, None).await
}

/// A node whose Access certs come from `certs` instead of Cloudflare.
pub async fn node_with(
    account: &str,
    runtime: Arc<dyn TeamRuntime>,
    certs: Option<Arc<dyn CertSource>>,
) -> Running {
    node_from(account, runtime, certs, None).await
}

/// A node that is the same on every run: its account key comes from `seed`, its random values count
/// up, and it listens on `port` (outside the ports the system hands out), so keys, signatures, ids, and
/// the URLs inside signed documents repeat.
pub async fn seeded_node(
    account: &str,
    runtime: Arc<dyn TeamRuntime>,
    seed: u8,
    port: u16,
) -> Running {
    node_from(account, runtime, None, Some((seed, port))).await
}

async fn node_from(
    account: &str,
    runtime: Arc<dyn TeamRuntime>,
    certs: Option<Arc<dyn CertSource>>,
    fixed: Option<(u8, u16)>,
) -> Running {
    let dir = TempDir::new().unwrap();
    let listener = match fixed {
        Some((_, port)) => TcpListener::bind(("127.0.0.1", port)).await.unwrap(),
        None => TcpListener::bind("127.0.0.1:0").await.unwrap(),
    };
    let port = listener.local_addr().unwrap().port();
    let seed = fixed.map(|(seed, _)| seed);
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    if let Some(seed) = seed {
        let key = agency_proto::AccountKey::from_seed([seed; 32]);
        std::fs::write(data.root().join("account.key"), key.encoded() + "\n").unwrap();
    }
    let url = format!("http://127.0.0.1:{port}");
    let account = Account {
        id: id(account),
        label: account.to_uppercase(),
    };
    data.save_config(&NodeConfig::new(
        url,
        format!("127.0.0.1:{port}"),
        account,
        "scrum",
    ))
    .unwrap();
    let clock = ManualClock::at(T0);
    let notifier = MemoryNotifier::default();
    let delivery = MemoryDelivery::default();
    let root_keys = Arc::new(seed.map_or_else(MemoryRootKeys::default, MemoryRootKeys::seeded));
    let overrides = Overrides {
        runtime: Some(runtime),
        clock: Some(Arc::new(clock.clone())),
        random: seed
            .map(|_| Arc::new(CountingRandom::default()) as Arc<dyn agency_core::ports::Random>),
        certs,
        notifier: Some(Arc::new(notifier.clone())),
        delivery: Some(Arc::new(delivery.clone())),
        launch: Some(Launch {
            executable: "/opt/agency/bin/agency".into(),
            data_dir: "/var/lib/agency".into(),
        }),
        root_keys: Some(root_keys.clone()),
    };
    let opened = agency_node::open(data, overrides).unwrap();
    let mut running = Running {
        node: opened.node.clone(),
        clock,
        notifier,
        delivery,
        root_keys,
        opened,
        port,
        listener: Some(listener),
        server: None,
        _dir: dir,
    };
    running.start().await;
    running
}

/// alice's node: holds the root key of org acme and hosts team app, which has a waiting request r-1.
pub async fn alice() -> (Running, Arc<Mutex<MemoryRuntimeState>>) {
    let (runtime, memory) = memory_runtime();
    let running = node("alice", runtime).await;
    (alice_on(running, &memory), memory)
}

/// Sets up alice's org and team on `running`, whose runtime keeps its requests in `memory`.
pub fn alice_on(running: Running, memory: &Arc<Mutex<MemoryRuntimeState>>) -> Running {
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-1".into(), "Make a key?".into()));
    let node = &running.node;
    orgs::create(node, id("acme"), "Acme".into()).unwrap();
    orgs::add_team(
        node,
        &id("acme"),
        TeamPlace::new(id("app"), "app", running.url()),
    )
    .unwrap();
    node.update_config(|config| {
        let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
        config
            .teams
            .push(HostedTeam::new(id("acme"), id("app"), id("alice"), runtime))
    })
    .unwrap();
    running
}

/// Makes bob the one to decide `request` in alice's team: he is the runtime name `reviewer` there, and
/// the request is addressed to that name. Only a request's addressee decides it.
pub fn for_bob(alice: &Running, memory: &Arc<Mutex<MemoryRuntimeState>>, request: &str) {
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("reviewer".into(), id("bob"));
        })
        .unwrap();
    let mut state = memory.lock().unwrap();
    state.to.insert(request.into(), "reviewer".into());
}

pub async fn joined_bob(alice: &Running) -> Running {
    joined(alice, node("bob", memory_runtime().0).await).await
}

/// `bob` after joining alice's org acme as a member.
pub async fn joined(alice: &Running, bob: Running) -> Running {
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let joined = join(&bob.node, &link, |definition| definition.label == "Acme")
        .await
        .unwrap();
    let agency_core::invites::Joined::Member(doc) = joined else {
        panic!("the join was held: {joined:?}")
    };
    assert!(doc.body.member(&id("bob")).is_some());
    bob
}

pub fn answer(choice: &str) -> Payload {
    Payload {
        text: String::new(),
        choice: Some(choice.into()),
    }
}

/// A request to a node's Client API, sent from loopback with its local `Host`.
pub fn get(running: &Running, path: &str) -> RequestBuilder {
    Client::new().get(format!("{}{path}", running.url()))
}

pub fn post(running: &Running, path: &str, body: &Value) -> RequestBuilder {
    Client::new()
        .post(format!("{}{path}", running.url()))
        .json(body)
}

pub async fn body(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// Adds a first device for the node's account, as `agency device-add` does, and returns its key.
pub async fn device(running: &Running) -> agency_proto::AccountKey {
    let key = agency_proto::AccountKey::generate();
    let added = agency_core::devices::add_device(&running.node, "Test", &key.public(), None).await;
    assert!(
        added
            .iter()
            .all(|(_, outcome)| matches!(outcome, agency_core::devices::Added::Done(_))),
        "{added:?}"
    );
    key
}

/// Decides in acme/app as the node's account, signed by its device `key`.
pub async fn decide_as(
    running: &Running,
    key: &agency_proto::AccountKey,
    target: &str,
    payload: Payload,
) -> agency_core::Result<agency_core::decisions::Decided> {
    let device = key.public().device_id();
    let signer = agency_core::devices::DeviceSigner { id: &device, key };
    let (org, team) = (id("acme"), id("app"));
    agency_core::signed_decisions::decide_with_device(
        &running.node,
        signer,
        &org,
        &team,
        agency_proto::EnvelopeKind::Answer,
        target.into(),
        payload,
    )
    .await
}

static NEXT_ENVELOPE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);

/// An answer to `target` in acme/app by the node's account, signed by its device `key`, made now.
pub fn signed_answer(
    running: &Running,
    key: &agency_proto::AccountKey,
    target: &str,
    choice: &str,
) -> agency_proto::Signed<agency_proto::Envelope> {
    sign_answer(running, key, target, answer(choice))
}

/// `signed_answer` with any payload.
pub fn sign_answer(
    running: &Running,
    key: &agency_proto::AccountKey,
    target: &str,
    payload: Payload,
) -> agency_proto::Signed<agency_proto::Envelope> {
    let next = NEXT_ENVELOPE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let body = agency_proto::Envelope {
        id: agency_proto::EnvelopeId::from_bytes([next; 16]),
        kind: agency_proto::EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: target.into(),
        payload,
        account: running.node.account(),
        at: running.node.now_text(),
    };
    agency_proto::Signed::sign(
        agency_proto::Kind::Envelope,
        body,
        key.public().device_id(),
        key,
    )
    .unwrap()
}

/// The Client API path for device-signed decisions in acme/app.
pub const ENVELOPES: &str = "/api/orgs/acme/teams/app/envelopes";
