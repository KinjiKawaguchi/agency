//! The node's data directory. Writes go to a temporary file that then replaces the target, so readers never
//! see a half-written file. The account key file and the directory are readable only by their owner.
use agency_core::config::NodeConfig;
use agency_core::decisions::DecisionRecord;
use agency_core::devices::DeviceRequestRecord;
use agency_core::invites::InviteRecord;
use agency_core::outbox::Parcel;
use agency_core::ports::{
    AppliedLog, ConfigStore, DecisionLogStore, DeviceRequestStore, InviteStore, OrgStore,
    OutboxStore, PushStore, ReplicaStore,
};
use agency_core::views::Replica;
use agency_core::{Error, Result};
use agency_proto::client::PushToken;
use agency_proto::org::SignedOrg;
use agency_proto::{AccountKey, EnvelopeId, Id};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;
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
}

impl InviteStore for DataDir {
    fn load_invite(&self, digest: &str) -> Option<InviteRecord> {
        read_json(&self.dir("invites").join(format!("{digest}.json")))
    }

    fn save_invite(&self, digest: &str, record: &InviteRecord) -> Result<()> {
        write_json(&self.dir("invites").join(format!("{digest}.json")), record)
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

    fn notified(&self) -> Option<Vec<String>> {
        read_json(&self.0.join("notified.json"))
    }

    fn save_notified(&self, keys: &[String]) -> Result<()> {
        write_json(&self.0.join("notified.json"), keys)
    }
}
