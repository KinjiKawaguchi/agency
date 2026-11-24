//! Agency's node: wires agency-core to its adapters. One node runs per machine; it serves the teams it
//! hosts, keeps replicas of teams on other nodes, and sends signed decisions through its outbox.
use agency_adapters::files::DataDir;
use agency_adapters::peers::HttpPeers;
use agency_adapters::runtime::CommandRuntime;
use agency_adapters::system::{OsRandom, SystemClock, TokioWriteLock};
use agency_core::ports::{Clock, Random, TeamRuntime};
use agency_core::{Node, Ports};
use std::sync::Arc;

/// Ports to use instead of the real ones (tests).
#[derive(Default)]
pub struct Overrides {
    pub runtime: Option<Arc<dyn TeamRuntime>>,
    pub clock: Option<Arc<dyn Clock>>,
    pub random: Option<Arc<dyn Random>>,
}

/// Opens the node whose configuration and account key live in `data`.
pub fn open(data: DataDir, overrides: Overrides) -> anyhow::Result<Arc<Node>> {
    let key = data.load_key()?;
    let ports = Ports {
        store: Arc::new(data),
        peers: Arc::new(HttpPeers::new()?),
        runtime: overrides
            .runtime
            .unwrap_or_else(|| Arc::new(CommandRuntime::default())),
        clock: overrides.clock.unwrap_or_else(|| Arc::new(SystemClock)),
        random: overrides.random.unwrap_or_else(|| Arc::new(OsRandom)),
        lock: Arc::new(TokioWriteLock::default()),
    };
    Ok(Arc::new(Node::open(ports, key)?))
}
