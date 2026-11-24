//! Nodes for the integration tests: each in its own data directory, on its own port, with a manual clock
//! at T0 and an in-memory team runtime.
#![allow(dead_code)]
use agency_adapters::access::CertSource;
use agency_adapters::client_api::Assets;
use agency_adapters::files::DataDir;
use agency_adapters::memory::{ManualClock, MemoryRuntime, MemoryRuntimeState};
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::invites::{self, join};
use agency_core::ports::{ConfigStore, TeamRuntime};
use agency_core::{orgs, Node};
use agency_node::{Opened, Overrides};
use agency_proto::org::TeamPlace;
use agency_proto::{Id, Payload, Role};
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
    opened: Opened,
    pub port: u16,
    server: Option<JoinHandle<()>>,
    _dir: TempDir,
}

impl Running {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The node's HTTP app, to call without a socket (with a chosen peer address).
    pub fn app(&self) -> axum::Router {
        self.opened
            .app_with(self.port, TOKEN.into(), Arc::new(TestWeb))
            .unwrap()
    }

    pub async fn start(&mut self) {
        let listener = TcpListener::bind(("127.0.0.1", self.port)).await.unwrap();
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
    let dir = TempDir::new().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let url = format!("http://127.0.0.1:{port}");
    data.save_config(&NodeConfig {
        url,
        listen: format!("127.0.0.1:{port}"),
        account: Account {
            id: id(account),
            label: account.to_uppercase(),
        },
        scrum: "scrum".into(),
        teams: vec![],
        trusted: vec![],
        funding: vec![],
        access: None,
    })
    .unwrap();
    let clock = ManualClock::at(T0);
    let overrides = Overrides {
        runtime: Some(runtime),
        clock: Some(Arc::new(clock.clone())),
        random: None,
        certs,
    };
    let opened = agency_node::open(data, overrides).unwrap();
    let mut running = Running {
        node: opened.node.clone(),
        clock,
        opened,
        port,
        server: None,
        _dir: dir,
    };
    running.start().await;
    running
}

/// alice の台: 組織 acme の根の鍵を持ち、チーム app を置く。チームには答え待ちの要求 r-1 がある
pub async fn alice() -> (Running, Arc<Mutex<MemoryRuntimeState>>) {
    let (runtime, memory) = memory_runtime();
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-1".into(), "鍵を作るか".into()));
    let running = node("alice", runtime).await;
    let node = &running.node;
    orgs::create(node, id("acme"), "Acme".into()).unwrap();
    orgs::add_team(
        node,
        &id("acme"),
        TeamPlace {
            id: id("app"),
            label: "app".into(),
            department: None,
            node: running.url(),
        },
    )
    .unwrap();
    node.update_config(|config| {
        config.teams.push(HostedTeam {
            org: id("acme"),
            team: id("app"),
            owner: id("alice"),
            people: Default::default(),
            runtime: RuntimeCommand {
                command: vec!["/nonexistent".into()],
                env: Default::default(),
            },
        })
    })
    .unwrap();
    (running, memory)
}

pub async fn joined_bob(alice: &Running) -> Running {
    let bob = node("bob", memory_runtime().0).await;
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let doc = join(&bob.node, &link, |definition| definition.label == "Acme")
        .await
        .unwrap();
    assert!(doc.body.member(&id("bob")).is_some());
    bob
}

pub fn answer(choice: &str) -> Payload {
    Payload {
        text: String::new(),
        choice: Some(choice.into()),
    }
}
