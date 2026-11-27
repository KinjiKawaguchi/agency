//! A node's shared state: its configuration, its account key, and the ports its use cases use.
use crate::config::{HostedTeam, NodeConfig};
use crate::error::Result;
use crate::ports::{
    Clock, DeliverySource, FundingSource, Journal, Notifier, Peers, Random, RootKeys, Store,
    TeamRuntime, WriteLock,
};
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::{AccountKey, Id};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

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
    pub journal: Arc<dyn Journal>,
    pub root_keys: Arc<dyn RootKeys>,
}

pub struct Node {
    ports: Ports,
    key: AccountKey,
    config: RwLock<NodeConfig>,
    /// The signed reads accepted recently, with when they were signed (see `accept_read_once`).
    reads: Mutex<HashMap<String, OffsetDateTime>>,
    /// Serializes the writes of the accounts that read bound (`change_bound_readers`).
    bound_readers_lock: Mutex<()>,
    /// Whether the owner has been told that the file of those accounts cannot be read.
    is_unreadable_said: AtomicBool,
    /// When each other node (by its origin as the node writes it) last answered this node, other than to a read of a
    /// team's view: a replica waiting for such a node is read at the next pass (`views`). Kept in memory only, and
    /// never written to a replica, so it cannot overwrite what a read saved.
    answers: Mutex<HashMap<String, OffsetDateTime>>,
}

/// An origin as the node writes it (`home_node_origin`), or as it was given when it is no such origin.
fn answer_key(origin: &str) -> String {
    agency_proto::client::home_node_origin(origin).unwrap_or_else(|| origin.to_owned())
}

/// What the journal says of a file of the accounts that read bound which cannot be read.
fn unreadable_line(error: &crate::error::Error) -> String {
    format!(
        "bound-readers.json cannot be read ({error}); it is left as it is, and reads that name no node are taken as before until it is mended or removed"
    )
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
            bound_readers_lock: Mutex::default(),
            is_unreadable_said: AtomicBool::new(false),
            answers: Mutex::default(),
        })
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

    /// The accounts whose node signs reads that name this node (see `read_auth`). When the file that keeps them
    /// cannot be read they are taken as none, so that reads keep being answered, and the owner is told once.
    pub(crate) fn bound_readers(&self) -> Result<BTreeSet<Id>> {
        self.store().bound_readers().inspect_err(|error| {
            if !self.is_unreadable_said.swap(true, Ordering::Relaxed) {
                self.journal().warn("federation", &unreadable_line(error));
            }
        })
    }

    /// Changes the accounts whose node signs reads that name this node, one change at a time, and keeps the
    /// result in the data directory when it differs. A file that cannot be read is left as it is, never written
    /// over, and said in the journal; a failure to keep the result is said too, and the accounts are then taken
    /// as before.
    pub(crate) fn change_bound_readers(&self, change: impl FnOnce(&mut BTreeSet<Id>)) {
        let _one_at_a_time = self
            .bound_readers_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let before = match self.store().bound_readers() {
            Ok(before) => before,
            Err(error) => {
                self.journal().warn("federation", &unreadable_line(&error));
                return;
            }
        };
        let mut after = before.clone();
        change(&mut after);
        if after == before {
            return;
        }
        if let Err(error) = self.store().save_bound_readers(&after) {
            self.journal().warn(
                "federation",
                &format!(
                    "could not keep which accounts read in the form that names this node: {error}"
                ),
            );
        }
    }

    /// Notes that the node at `origin` has just answered this node.
    pub(crate) fn note_answer(&self, origin: &str) {
        let key = answer_key(origin);
        let mut answers = self
            .answers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        answers.insert(key, self.now());
    }

    /// When the node at `origin` last answered this node, other than to a read of a team's view.
    pub(crate) fn answered_at(&self, origin: &str) -> Option<OffsetDateTime> {
        let answers = self
            .answers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        answers.get(&answer_key(origin)).copied()
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

    pub(crate) fn journal(&self) -> &dyn Journal {
        self.ports.journal.as_ref()
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
