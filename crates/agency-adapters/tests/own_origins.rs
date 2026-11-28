//! The origins a node keeps as its own: the ones it was configured with at each start and the last twenty, and
//! a file of them that cannot be read.
mod node_fixture;

use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::memory::{CountingRandom, ManualClock, MemoryRuntime, NoNotifier};
use agency_adapters::peers::HttpPeers;
use agency_adapters::system::TokioWriteLock;
use agency_core::config::{Account, NodeConfig};
use agency_core::{Node, Ports};
use node_fixture::*;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn a_node_keeps_the_origins_it_was_configured_with_at_each_start_and_the_last_twenty() {
    use agency_core::ports::ConfigStore as _;
    let dir = TempDir::new().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let open = |url: &str| {
        let account = Account {
            id: id("alice"),
            label: "ALICE".into(),
        };
        data.save_config(&NodeConfig::new(
            url.to_owned(),
            "127.0.0.1:0",
            account,
            "scrum",
        ))
        .unwrap();
        let ports = Ports {
            store: Arc::new(DataDir::new(dir.path().join("node"))),
            peers: Arc::new(HttpPeers::new().unwrap()),
            runtime: Arc::new(MemoryRuntime::default()),
            clock: Arc::new(ManualClock::at(T0)),
            random: Arc::new(CountingRandom::default()),
            lock: Arc::new(TokioWriteLock::default()),
            funding: Arc::new(UsageFiles::new(dir.path().join("node"))),
            delivery: Arc::new(agency_adapters::memory::MemoryDelivery::default()),
            notifier: Arc::new(NoNotifier),
            journal: Arc::new(agency_adapters::memory::NoJournal),
            root_keys: Arc::new(agency_adapters::memory::MemoryRootKeys::default()),
        };
        Node::open(ports, data.load_key().unwrap()).unwrap()
    };

    assert_eq!(
        open("https://One.Example.com:443/").own_origins(),
        ["https://one.example.com"]
    );
    assert_eq!(
        open("https://two.example.com").own_origins(),
        ["https://one.example.com", "https://two.example.com"],
        "added at the start, as written one way"
    );
    assert_eq!(
        open("https://one.example.com").own_origins().len(),
        2,
        "an origin it had is not added again"
    );
    assert_eq!(
        open("not an origin").own_origins().len(),
        2,
        "a url that is no origin is not kept"
    );

    for n in 0..25 {
        open(&format!("https://node-{n}.example.com"));
    }
    let kept = open("https://last.example.com").own_origins();
    assert_eq!(kept.len(), 20, "{kept:?}");
    assert_eq!(kept[19], "https://last.example.com");
    assert_eq!(
        kept[0], "https://node-6.example.com",
        "the oldest are forgotten first"
    );
    assert!(!kept.contains(&"https://one.example.com".to_owned()));
}

/// A journal that keeps what it is told, to read back.
#[derive(Default)]
struct Said(std::sync::Mutex<Vec<String>>);

impl agency_core::ports::Journal for Said {
    fn info(&self, _event: &str, _line: &str) {}

    fn warn(&self, _event: &str, line: &str) {
        self.0.lock().unwrap().push(line.to_owned());
    }
}

#[tokio::test]
async fn a_file_of_origins_that_cannot_be_read_is_left_as_it_is_said_once_and_only_the_current_origin_counts(
) {
    use agency_core::ports::ConfigStore as _;
    let dir = TempDir::new().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    data.save_config(&NodeConfig::new(
        "https://one.example.com",
        "127.0.0.1:0",
        account,
        "scrum",
    ))
    .unwrap();
    let path = dir.path().join("node").join("own-origins.json");
    std::fs::write(&path, "not json\n").unwrap();
    let said = Arc::new(Said::default());
    let ports = Ports {
        store: Arc::new(DataDir::new(dir.path().join("node"))),
        peers: Arc::new(HttpPeers::new().unwrap()),
        runtime: Arc::new(MemoryRuntime::default()),
        clock: Arc::new(ManualClock::at(T0)),
        random: Arc::new(CountingRandom::default()),
        lock: Arc::new(TokioWriteLock::default()),
        funding: Arc::new(UsageFiles::new(dir.path().join("node"))),
        delivery: Arc::new(agency_adapters::memory::MemoryDelivery::default()),
        notifier: Arc::new(NoNotifier),
        journal: said.clone(),
        root_keys: Arc::new(agency_adapters::memory::MemoryRootKeys::default()),
    };
    let node = Node::open(ports, data.load_key().unwrap()).unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "not json\n",
        "not written over"
    );
    assert_eq!(
        node.own_origins(),
        ["https://one.example.com"],
        "only the one it is configured with now"
    );
    assert_eq!(node.own_origins(), ["https://one.example.com"]);
    let lines = said.0.lock().unwrap().clone();
    assert_eq!(lines.len(), 1, "said once: {lines:?}");
    assert!(
        lines[0].contains("own-origins.json cannot be read"),
        "{lines:?}"
    );

    // Once it is mended (here: removed), the origin is kept as at any start.
    std::fs::remove_file(&path).unwrap();
    node.update_config(|config| config.url = "https://two.example.com".into())
        .unwrap();
    assert_eq!(
        node.own_origins(),
        ["https://two.example.com"],
        "the file starts again with the current origin"
    );
}
