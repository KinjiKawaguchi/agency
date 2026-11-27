//! The node's data directory. Writes go to a temporary file that then replaces the target, so readers never
//! see a half-written file. The account key file and the directory are readable only by their owner.
use agency_core::claims::ClaimRecord;
use agency_core::clients_seen::ClientsSeen;
use agency_core::config::NodeConfig;
use agency_core::decisions::DecisionRecord;
use agency_core::devices::DeviceRequestRecord;
use agency_core::guests::Guest;
use agency_core::invites::InviteRecord;
use agency_core::outbox::Parcel;
use agency_core::peer_about::PeerAbout;
use agency_core::pending_root::{PendingRootChange, WaitingJoin};
use agency_core::ports::{
    AboutStore, AppliedLog, BoundReaderStore, ClaimStore, ClientsSeenStore, ConfigStore,
    DecisionLogStore, DeviceRequestStore, GuestStore, InviteStore, OrgStore, OutboxStore,
    OutcomeStore, PendingRootStore, PushStore, ReplicaStore, ShipLedger, ShipRecord,
};
use agency_core::push_retries::Retry;
use agency_core::views::Replica;
use agency_core::web_push::WebSubscription;
use agency_core::{Error, Result};
use agency_proto::client::{GuestRef, PushToken};
use agency_proto::digest::sha256_hex;
use agency_proto::observation::Observation;
use agency_proto::org::SignedOrg;
use agency_proto::{AccountKey, EnvelopeId, Id};
use serde::{de::DeserializeOwned, Serialize};
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn io_error(path: &Path, error: impl std::fmt::Display) -> Error {
    Error::internal(format!("{}: {error}", path.display()))
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io_error(path, "no parent directory"))?;
    fs::create_dir_all(dir).map_err(|error| io_error(dir, error))?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let text = serde_json::to_string_pretty(value).map_err(|error| io_error(path, error))? + "\n";
    fs::write(&tmp, text).map_err(|error| io_error(&tmp, error))?;
    fs::rename(&tmp, path).map_err(|error| io_error(path, error))
}

/// Reads every `.json` file in a directory, in name order, skipping files that do not parse.
pub fn read_all<T: DeserializeOwned>(dir: &Path) -> Vec<T> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths.iter().filter_map(|path| read_json(path)).collect()
}

fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(io_error(path, error)),
        _ => Ok(()),
    }
}

#[derive(Clone, Debug)]
pub struct DataDir(PathBuf);

impl DataDir {
    pub fn new(root: PathBuf) -> Self {
        Self(root)
    }

    pub fn root(&self) -> &Path {
        &self.0
    }

    pub fn config_path(&self) -> PathBuf {
        self.0.join("config.json")
    }

    fn key_path(&self) -> PathBuf {
        self.0.join("account.key")
    }

    pub fn load_key(&self) -> Result<AccountKey> {
        let path = self.key_path();
        let raw = fs::read_to_string(&path).map_err(|error| io_error(&path, error))?;
        AccountKey::from_encoded(&raw)
            .ok_or_else(|| Error::internal("the account key file is malformed"))
    }

    pub fn create_key(&self) -> Result<AccountKey> {
        let private = fs::Permissions::from_mode(0o700);
        fs::create_dir_all(&self.0)
            .and_then(|()| fs::set_permissions(&self.0, private))
            .map_err(|error| io_error(&self.0, error))?;
        let key = AccountKey::generate();
        let path = self.key_path();
        fs::write(&path, key.encoded() + "\n")
            .and_then(|()| fs::set_permissions(&path, fs::Permissions::from_mode(0o600)))
            .map_err(|error| io_error(&path, error))?;
        Ok(key)
    }

    fn device_key_path(&self) -> PathBuf {
        self.0.join("cli-device.key")
    }

    /// The CLI device's key, if this node has one.
    pub fn load_device_key(&self) -> Result<Option<AccountKey>> {
        let path = self.device_key_path();
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path).map_err(|error| io_error(&path, error))?;
        AccountKey::from_encoded(&raw)
            .map(Some)
            .ok_or_else(|| Error::internal("the CLI device key file is malformed"))
    }

    /// Makes the CLI device's key. The file is created readable only by its owner, and never replaced.
    pub fn create_device_key(&self) -> Result<AccountKey> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let key = AccountKey::generate();
        let path = self.device_key_path();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut file| file.write_all((key.encoded() + "\n").as_bytes()))
            .map_err(|error| io_error(&path, error))?;
        Ok(key)
    }

    fn dir(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn outcome_path(&self, org: &Id) -> PathBuf {
        self.dir("outcomes").join(format!("{org}.jsonl"))
    }

    fn ships_path(&self, org: &Id) -> PathBuf {
        self.dir("outcomes").join(format!("{org}.ships.jsonl"))
    }

    /// Appends one line to a file of the `outcomes` directory, which only this account can read: the
    /// directory is 0700 and the file 0600.
    fn append_private_line(&self, path: &Path, line: &str) -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let dir = self.dir("outcomes");
        fs::create_dir_all(&dir)
            .and_then(|()| fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)))
            .map_err(|error| io_error(&dir, error))?;
        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(path)
            .and_then(|mut file| {
                file.write_all(line.as_bytes())?;
                file.write_all(b"\n")?;
                file.sync_all()
            })
            .map_err(|error| io_error(path, error))
    }

    fn guest_notified_path(&self, guest: &GuestRef) -> PathBuf {
        let name: String =
            sha2::Sha256::digest(format!("{}/{}", guest.org, guest.member).as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
        self.dir("guests")
            .join("notified")
            .join(format!("{name}.json"))
    }

    fn guest_path(&self, email: &str) -> PathBuf {
        let name: String = sha2::Sha256::digest(email.trim().to_lowercase().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.dir("guests").join(format!("{name}.json"))
    }

    fn recovery_start_path(&self, org: &Id, version: u64) -> PathBuf {
        self.dir("orgs")
            .join(format!("{org}.history"))
            .join(format!("{version:06}.recovery-start.json"))
    }

    /// A held change's file, named by the SHA-256 of its handle: a handle is only ever compared, never
    /// listed in a path an error could show.
    fn pending_root_path(&self, id: &str) -> PathBuf {
        let name: String = sha2::Sha256::digest(id.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.dir("pending-root").join(format!("{name}.json"))
    }

    fn org_version_path(&self, org: &Id, version: u64) -> PathBuf {
        self.dir("orgs")
            .join(format!("{org}.history"))
            .join(format!("{version:06}.json"))
    }

    fn org_path(&self, org: &Id) -> PathBuf {
        self.dir("orgs").join(format!("{org}.json"))
    }

    fn applied_path(&self) -> PathBuf {
        self.0.join("applied.json")
    }

    fn applied(&self) -> BTreeMap<String, String> {
        read_json(&self.applied_path()).unwrap_or_default()
    }
}

impl ConfigStore for DataDir {
    fn load_config(&self) -> Result<NodeConfig> {
        let path = self.config_path();
        read_json(&path).ok_or_else(|| {
            Error::internal(format!(
                "cannot read {} (create it with agency init)",
                path.display()
            ))
        })
    }

    fn save_config(&self, config: &NodeConfig) -> Result<()> {
        write_json(&self.config_path(), config)
    }
}

impl OrgStore for DataDir {
    fn load_org(&self, org: &Id) -> Option<SignedOrg> {
        read_json(&self.org_path(org))
    }

    fn load_org_version(&self, org: &Id, version: u64) -> Option<SignedOrg> {
        read_json(&self.org_version_path(org, version))
    }

    fn save_org(&self, doc: &SignedOrg) -> Result<()> {
        let history = self.org_version_path(&doc.body.org, doc.body.version);
        write_json(&history, doc)?;
        write_json(&self.org_path(&doc.body.org), doc)
    }

    fn orgs(&self) -> Vec<SignedOrg> {
        read_all(&self.dir("orgs"))
    }

    fn note_recovery_start(&self, org: &Id, version: u64, at: &str) -> Result<()> {
        write_json(&self.recovery_start_path(org, version), at)
    }

    fn recovery_started(&self, org: &Id, version: u64) -> Option<String> {
        read_json(&self.recovery_start_path(org, version))
    }
}

impl PendingRootStore for DataDir {
    fn put_pending_root(&self, change: &PendingRootChange) -> Result<()> {
        write_json(&self.pending_root_path(&change.id), change)
    }

    fn pending_roots(&self) -> Vec<PendingRootChange> {
        read_all(&self.dir("pending-root"))
    }

    fn remove_pending_root(&self, id: &str) -> Result<()> {
        remove(&self.pending_root_path(id))
    }

    fn put_waiting_join(&self, join: &WaitingJoin) -> Result<()> {
        write_json(
            &self.dir("waiting-joins").join(format!("{}.json", join.org)),
            join,
        )
    }

    fn waiting_joins(&self) -> Vec<WaitingJoin> {
        read_all(&self.dir("waiting-joins"))
    }

    fn remove_waiting_join(&self, org: &Id) -> Result<()> {
        remove(&self.dir("waiting-joins").join(format!("{org}.json")))
    }
}

impl InviteStore for DataDir {
    fn load_invite(&self, digest: &str) -> Option<InviteRecord> {
        read_json(&self.dir("invites").join(format!("{digest}.json")))
    }

    fn save_invite(&self, digest: &str, record: &InviteRecord) -> Result<()> {
        write_json(&self.dir("invites").join(format!("{digest}.json")), record)
    }
}

impl ClaimStore for DataDir {
    fn load_claim(&self, digest: &str) -> Option<ClaimRecord> {
        read_json(&self.dir("claims").join(format!("{digest}.json")))
    }

    /// Every file of `claims/`, named by the digest it holds.
    fn claims(&self) -> Vec<(String, ClaimRecord)> {
        let Ok(entries) = fs::read_dir(self.dir("claims")) else {
            return vec![];
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let digest = path.file_stem()?.to_str()?.to_owned();
                let is_json = path.extension().is_some_and(|ext| ext == "json");
                is_json.then(|| read_json(&path).map(|record| (digest, record)))?
            })
            .collect()
    }

    fn claim_for_seat(&self, org: &Id, seat: &Id) -> Option<String> {
        read_json(&self.dir("claims-by-seat").join(format!(
            "{}-{}.json",
            sha256_hex(org.as_str().as_bytes()),
            sha256_hex(seat.as_str().as_bytes())
        )))
    }

    fn save_claim(&self, digest: &str, record: &ClaimRecord) -> Result<()> {
        write_json(&self.dir("claims").join(format!("{digest}.json")), record)
    }

    fn remove_claim(&self, digest: &str) -> Result<()> {
        remove(&self.dir("claims").join(format!("{digest}.json")))
    }

    fn save_claim_for_seat(&self, org: &Id, seat: &Id, digest: &str) -> Result<()> {
        write_json(
            &self.dir("claims-by-seat").join(format!(
                "{}-{}.json",
                sha256_hex(org.as_str().as_bytes()),
                sha256_hex(seat.as_str().as_bytes())
            )),
            &digest,
        )
    }
}

impl AppliedLog for DataDir {
    fn is_applied(&self, id: &EnvelopeId) -> bool {
        self.applied().contains_key(id.as_str())
    }

    fn record_applied(&self, id: &EnvelopeId, at: &str) -> Result<()> {
        let mut applied = self.applied();
        applied.insert(id.as_str().to_owned(), at.to_owned());
        write_json(&self.applied_path(), &applied)
    }
}

impl OutboxStore for DataDir {
    fn put_parcel(&self, parcel: &Parcel) -> Result<()> {
        write_json(
            &self.dir("outbox").join(format!("{}.json", parcel.id)),
            parcel,
        )
    }

    fn pending_parcels(&self) -> Vec<Parcel> {
        read_all(&self.dir("outbox"))
    }

    fn remove_parcel(&self, id: &str) -> Result<()> {
        remove(&self.dir("outbox").join(format!("{id}.json")))
    }

    fn refuse_parcel(&self, parcel: &Parcel) -> Result<()> {
        write_json(
            &self.dir("refused").join(format!("{}.json", parcel.id)),
            parcel,
        )
    }

    fn refused_parcels(&self) -> Vec<Parcel> {
        read_all(&self.dir("refused"))
    }
}

impl GuestStore for DataDir {
    /// `guests/<sha256 of the email>.json`: an email is never a file name.
    fn guest(&self, email: &str) -> Option<Guest> {
        read_json(&self.guest_path(email))
    }

    fn put_guest(&self, guest: &Guest) -> Result<()> {
        write_json(&self.guest_path(&guest.email), guest)
    }

    fn remove_guest(&self, email: &str) -> Result<()> {
        remove(&self.guest_path(email))
    }

    /// `guests/notified/<sha256 of org/member>.json`.
    fn guest_notified(&self, guest: &GuestRef) -> Option<Vec<String>> {
        read_json(&self.guest_notified_path(guest))
    }

    fn save_guest_notified(&self, guest: &GuestRef, keys: &[String]) -> Result<()> {
        write_json(&self.guest_notified_path(guest), &keys)
    }
}

impl ClientsSeenStore for DataDir {
    /// `clients-seen.json`: no account and no device in it, so it is no more private than the rest.
    fn clients_seen(&self) -> ClientsSeen {
        read_json(&self.0.join("clients-seen.json")).unwrap_or_default()
    }

    fn save_clients_seen(&self, seen: &ClientsSeen) -> Result<()> {
        write_json(&self.0.join("clients-seen.json"), seen)
    }
}

impl AboutStore for DataDir {
    /// `about/<org>.json`: one file for the org, since an origin is not a file name.
    fn peer_abouts(&self, org: &Id) -> BTreeMap<String, PeerAbout> {
        read_json(&self.dir("about").join(format!("{org}.json"))).unwrap_or_default()
    }

    fn save_peer_abouts(&self, org: &Id, abouts: &BTreeMap<String, PeerAbout>) -> Result<()> {
        write_json(&self.dir("about").join(format!("{org}.json")), abouts)
    }
}

impl BoundReaderStore for DataDir {
    /// `bound-readers.json`: account ids only, no key and no signature. A file that is not there is no accounts;
    /// one that is there and cannot be read or parsed is an error, so that nothing writes over it.
    fn bound_readers(&self) -> Result<BTreeSet<Id>> {
        let path = self.0.join("bound-readers.json");
        match fs::read_to_string(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
            Err(error) => Err(io_error(&path, error)),
            Ok(text) => serde_json::from_str(&text).map_err(|error| io_error(&path, error)),
        }
    }

    fn save_bound_readers(&self, accounts: &BTreeSet<Id>) -> Result<()> {
        write_json(&self.0.join("bound-readers.json"), accounts)
    }
}

impl ReplicaStore for DataDir {
    fn load_replica(&self, org: &Id, team: &Id) -> Option<Replica> {
        read_json(
            &self
                .dir("replicas")
                .join(org.as_str())
                .join(format!("{team}.json")),
        )
    }

    fn save_replica(&self, org: &Id, team: &Id, replica: &Replica) -> Result<()> {
        write_json(
            &self
                .dir("replicas")
                .join(org.as_str())
                .join(format!("{team}.json")),
            replica,
        )
    }

    fn delete_replica(&self, org: &Id, team: &Id) -> Result<()> {
        let path = self
            .dir("replicas")
            .join(org.as_str())
            .join(format!("{team}.json"));
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(Error::internal(
                format!("cannot remove {}: {error}", path.display()),
            )),
            _ => Ok(()),
        }
    }
}

/// How many decisions the log keeps; older ones are dropped.
const DECISION_LOG_SIZE: usize = 500;

impl DecisionLogStore for DataDir {
    fn append_decision(&self, record: &DecisionRecord) -> Result<()> {
        let path = self.0.join("decisions.json");
        let mut records: Vec<DecisionRecord> = read_json(&path).unwrap_or_default();
        records.push(record.clone());
        let keep = records.len().saturating_sub(DECISION_LOG_SIZE);
        write_json(&path, &records[keep..])
    }

    fn recent_decisions(&self, limit: usize) -> Vec<DecisionRecord> {
        let records: Vec<DecisionRecord> =
            read_json(&self.0.join("decisions.json")).unwrap_or_default();
        records.into_iter().rev().take(limit).collect()
    }
}

impl OutcomeStore for DataDir {
    /// The lines of `outcomes/<org>.jsonl`. A line that does not parse is skipped: it is never shown, since it
    /// may hold a business number.
    fn observations(&self, org: &Id) -> Vec<Observation> {
        let Ok(text) = fs::read_to_string(self.outcome_path(org)) else {
            return vec![];
        };
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn append_observation(&self, org: &Id, observation: &Observation) -> Result<()> {
        let path = self.outcome_path(org);
        let line = serde_json::to_string(observation).map_err(|error| io_error(&path, error))?;
        self.append_private_line(&path, &line)
    }
}

impl ShipLedger for DataDir {
    /// The lines of `outcomes/<org>.ships.jsonl`; a line that does not parse is skipped.
    fn ships(&self, org: &Id) -> Vec<ShipRecord> {
        let Ok(text) = fs::read_to_string(self.ships_path(org)) else {
            return vec![];
        };
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn append_ship(&self, org: &Id, record: &ShipRecord) -> Result<()> {
        let path = self.ships_path(org);
        let line = serde_json::to_string(record).map_err(|error| io_error(&path, error))?;
        self.append_private_line(&path, &line)
    }
}

impl DeviceRequestStore for DataDir {
    fn device_requests(&self) -> Vec<DeviceRequestRecord> {
        read_json(&self.0.join("device-requests.json")).unwrap_or_default()
    }

    fn save_device_requests(&self, requests: &[DeviceRequestRecord]) -> Result<()> {
        write_json(&self.0.join("device-requests.json"), requests)
    }
}

impl PushStore for DataDir {
    fn push_tokens(&self) -> Vec<PushToken> {
        read_json(&self.0.join("push-tokens.json")).unwrap_or_default()
    }

    fn save_push_tokens(&self, tokens: &[PushToken]) -> Result<()> {
        write_json(&self.0.join("push-tokens.json"), tokens)
    }

    fn push_retries(&self) -> Vec<Retry> {
        read_json(&self.0.join("push-retries.json")).unwrap_or_default()
    }

    fn save_push_retries(&self, retries: &[Retry]) -> Result<()> {
        write_json(&self.0.join("push-retries.json"), retries)
    }

    fn web_subscriptions(&self) -> Vec<WebSubscription> {
        read_json(&self.0.join("web-push-subscriptions.json")).unwrap_or_default()
    }

    fn save_web_subscriptions(&self, subscriptions: &[WebSubscription]) -> Result<()> {
        write_json(&self.0.join("web-push-subscriptions.json"), subscriptions)
    }

    fn notified(&self) -> Option<Vec<String>> {
        read_json(&self.0.join("notified.json"))
    }

    fn save_notified(&self, keys: &[String]) -> Result<()> {
        write_json(&self.0.join("notified.json"), keys)
    }

    fn desk_refresh(&self) -> Option<agency_core::push::DeskRefresh> {
        read_json(&self.0.join("desk-refresh.json"))
    }

    fn save_desk_refresh(&self, state: &agency_core::push::DeskRefresh) -> Result<()> {
        write_json(&self.0.join("desk-refresh.json"), state)
    }
}
