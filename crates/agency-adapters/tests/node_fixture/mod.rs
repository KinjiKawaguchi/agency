//! The node the use-case tests run: real files and a manual clock, with alice's org and team or a bare node, and
//! an envelope signed by alice's laptop. Each test file uses a part of it.
#![allow(dead_code)]
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::memory::{CountingRandom, ManualClock, MemoryRuntime, NoNotifier};
use agency_adapters::peers::HttpPeers;
use agency_adapters::system::TokioWriteLock;
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::orgs;
use agency_core::ports::ConfigStore;
use agency_core::{Node, Ports};
use agency_proto::org::TeamPlace;
use agency_proto::{
    AccountKey, Device, Envelope, EnvelopeId, EnvelopeKind, Id, Kind, Payload, Signed, Timestamp,
};
use std::sync::Arc;
use tempfile::TempDir;

pub const T0: &str = "2026-11-24T12:00:00Z";
/// Nothing listens here, so every delivery is unreachable.
pub const NOWHERE: &str = "http://127.0.0.1:1";

pub fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

pub struct Fixture {
    pub node: Node,
    pub clock: ManualClock,
    pub runtime: MemoryRuntime,
    _dir: TempDir,
}

/// alice's node at T0: the root of org acme with one device, hosting team app with one waiting request r-1.
pub fn alice() -> Fixture {
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
pub fn laptop() -> AccountKey {
    AccountKey::from_seed([3; 32])
}

/// A node at T0 with an account and nothing else.
pub fn bare(account: &str) -> Fixture {
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

pub fn envelope(at: &str, byte: u8) -> Signed<Envelope> {
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
