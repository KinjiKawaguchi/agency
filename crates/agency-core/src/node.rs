//! A node's shared state: its configuration, its account key, and the ports its use cases use.
use crate::config::NodeConfig;
use crate::error::Result;
use crate::ports::{Clock, Peers, Random, Store, TeamRuntime, WriteLock};
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::{AccountKey, Id};
use std::sync::{Arc, RwLock};

pub struct Ports {
    pub store: Arc<dyn Store>,
    pub peers: Arc<dyn Peers>,
    pub runtime: Arc<dyn TeamRuntime>,
    pub clock: Arc<dyn Clock>,
    pub random: Arc<dyn Random>,
    pub lock: Arc<dyn WriteLock>,
}

pub struct Node {
    ports: Ports,
    key: AccountKey,
    config: RwLock<NodeConfig>,
}

impl Node {
    /// `key` is this node's account key; it never leaves the node.
    pub fn open(ports: Ports, key: AccountKey) -> Result<Self> {
        let config = ports.store.load_config()?;
        Ok(Self {
            ports,
            key,
            config: RwLock::new(config),
        })
    }

    pub fn store(&self) -> &dyn Store {
        self.ports.store.as_ref()
    }

    pub(crate) fn peers(&self) -> &dyn Peers {
        self.ports.peers.as_ref()
    }

    pub(crate) fn runtime(&self) -> &dyn TeamRuntime {
        self.ports.runtime.as_ref()
    }

    pub(crate) fn random(&self) -> [u8; 16] {
        self.ports.random.bytes16()
    }

    pub(crate) fn write_lock(&self) -> &dyn WriteLock {
        self.ports.lock.as_ref()
    }

    pub fn key(&self) -> &AccountKey {
        &self.key
    }

    pub fn now(&self) -> OffsetDateTime {
        self.ports.clock.now()
    }

    /// The current time as RFC 3339 UTC text.
    pub fn now_text(&self) -> String {
        clock::format(self.now())
    }

    pub fn config(&self) -> NodeConfig {
        self.config
            .read()
            .map(|one| one.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Edits the configuration and saves it.
    pub fn update_config(&self, edit: impl FnOnce(&mut NodeConfig)) -> Result<()> {
        let mut guard = self
            .config
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        edit(&mut guard);
        self.ports.store.save_config(&guard)
    }

    pub fn account(&self) -> Id {
        self.config().account.id
    }

    pub fn url(&self) -> String {
        self.config().url
    }
}
