//! A node's shared state: its configuration, its account key, and the ports its use cases use.
use crate::config::{HostedTeam, NodeConfig};
use crate::error::Result;
use crate::ports::{
    Clock, DeliverySource, FundingSource, Notifier, OutboxBell, Peers, Random, RootKeys, Store,
    TeamRuntime, WriteLock,
};
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::{AccountKey, Id};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};

pub struct Ports {
    pub store: Arc<dyn Store>,
    pub peers: Arc<dyn Peers>,
    pub runtime: Arc<dyn TeamRuntime>,
    pub clock: Arc<dyn Clock>,
    pub random: Arc<dyn Random>,
    pub lock: Arc<dyn WriteLock>,
    pub funding: Arc<dyn FundingSource>,
    pub delivery: Arc<dyn DeliverySource>,
    pub notifier: Arc<dyn Notifier>,
    pub root_keys: Arc<dyn RootKeys>,
}

pub struct Node {
    ports: Ports,
    key: AccountKey,
    config: RwLock<NodeConfig>,
    /// The signed reads accepted recently, with when they were signed (see `accept_read_once`).
    reads: Mutex<HashMap<String, OffsetDateTime>>,
    /// Rung when a parcel is queued for the background (see `set_outbox_bell`).
    bell: OnceLock<Arc<dyn OutboxBell>>,
    /// Held while a decision is checked against what the node already kept and queued, so that two sends
    /// of one envelope keep one parcel. Never held across an await.
    intake: Mutex<()>,
}

/// At most this many recent signed reads are remembered. A signature is remembered only once it is
/// valid, so only members fill it; beyond it, the oldest are forgotten first.
const READS_KEPT: usize = 10_000;

impl Node {
    /// `key` is this node's account key; it never leaves the node.
    pub fn open(ports: Ports, key: AccountKey) -> Result<Self> {
        let config = ports.store.load_config()?;
        Ok(Self {
            ports,
            key,
            config: RwLock::new(config),
            reads: Mutex::default(),
            bell: OnceLock::new(),
            intake: Mutex::new(()),
        })
    }

    /// Sets what is rung when a parcel is queued. Set once, by the process that delivers the outbox.
    pub fn set_outbox_bell(&self, bell: Arc<dyn OutboxBell>) {
        let _ = self.bell.set(bell);
    }

    pub(crate) fn ring_outbox_bell(&self) {
        if let Some(bell) = self.bell.get() {
            bell.ring();
        }
    }

    pub(crate) fn intake(&self) -> MutexGuard<'_, ()> {
        self.intake
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Accepts a valid signed read once. A signed read is good for `SKEW_SECONDS` either way of its
    /// time, so a copy of it could be sent again; the same signature within that window is refused.
    /// Entries are dropped once their time is out of the window.
    pub fn accept_read_once(&self, signature: &str, signed_at: OffsetDateTime) -> bool {
        let window = clock::Duration::seconds(crate::read_auth::SKEW_SECONDS);
        let now = self.now();
        let mut reads = self
            .reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reads.retain(|_, at| (now - *at).abs() <= window);
        if reads.contains_key(signature) {
            return false;
        }
        if reads.len() >= READS_KEPT {
            let oldest = reads
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                reads.remove(&oldest);
            }
        }
        reads.insert(signature.to_owned(), signed_at);
        true
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

    pub(crate) fn funding(&self) -> &dyn FundingSource {
        self.ports.funding.as_ref()
    }

    pub(crate) fn delivery(&self) -> &dyn DeliverySource {
        self.ports.delivery.as_ref()
    }

    pub(crate) fn notifier(&self) -> &dyn Notifier {
        self.ports.notifier.as_ref()
    }

    pub(crate) fn root_keys(&self) -> &dyn RootKeys {
        self.ports.root_keys.as_ref()
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

    /// The node's time zone, as the offset from UTC now.
    pub fn utc_offset(&self) -> clock::UtcOffset {
        self.ports.clock.utc_offset()
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

    /// The team, if this node serves it: it is in the node's own hosted-team configuration, and in the
    /// org's definition when the node has one. The org's `TeamPlace.node` never decides, since it is only
    /// where other nodes reach the team and can lag behind a change of this node's URL. A team the org
    /// removed stays in the configuration until its owner takes it out, and is not served meanwhile.
    pub fn hosted(&self, org: &Id, team: &Id) -> Option<HostedTeam> {
        let hosted = self.config().hosted(org, team).cloned()?;
        let is_listed = self
            .store()
            .load_org(org)
            .is_none_or(|doc| doc.body.team(team).is_some());
        is_listed.then_some(hosted)
    }

    pub fn runs_here(&self, org: &Id, team: &Id) -> bool {
        self.hosted(org, team).is_some()
    }

    pub fn account(&self) -> Id {
        self.config().account.id
    }

    pub fn url(&self) -> String {
        self.config().url
    }
}
