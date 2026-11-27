//! Today's node on the data directories that earlier releases wrote (`testdata/releases/<tag>/data/<node>/`,
//! made by `scripts/release/snapshot-data.sh`). Upgrades are forward-only (docs/compatibility.md, "Node data
//! and configuration"), so each directory must open, its org definitions must verify, and everything the
//! release kept in it must read. A file today's code cannot parse is skipped by the stores, so the check
//! counts what is on disk against what the stores give back, and refuses a file it has no check for.
use agency_adapters::files::DataDir;
use agency_adapters::memory::{ManualClock, MemoryDelivery, MemoryRootKeys, MemoryRuntime};
use agency_core::config::NodeConfig;
use agency_core::ports::{
    AppliedLog, ConfigStore, DecisionLogStore, InviteStore, OrgStore, PushStore, ReplicaStore,
};
use agency_node::Overrides;
use agency_proto::org::{verify_genesis, verify_successor, SignedOrg};
use agency_proto::{EnvelopeId, Id, Kind};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a node's data directory holds at its top level, as each release writes it. A name that is not
/// here fails the check, so that a snapshot of a release that writes something new is read too.
const READ_BELOW: &[&str] = &[
    "account.key",
    "applied.json",
    "cli-device.key",
    "config.json",
    "decisions.json",
    "desk-refresh.json",
    "format",
    "invites",
    "notified.json",
    "orgs",
    "replicas",
];

fn releases_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/releases")
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    paths
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

/// Every node data directory kept for an earlier release, as `(tag, node, path)`.
fn snapshots() -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    for release in sorted_entries(&releases_dir()) {
        let data = release.join("data");
        if !data.is_dir() {
            continue;
        }
        for node in sorted_entries(&data) {
            found.push((file_name(&release), file_name(&node), node));
        }
    }
    found
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in sorted_entries(from) {
        let target = to.join(file_name(&entry));
        if entry.is_dir() {
            copy_tree(&entry, &target);
        } else {
            std::fs::copy(&entry, &target).unwrap();
        }
    }
}

/// A copy of a snapshot in a temporary directory, which the node may write to and a test may damage.
fn copy_of(snapshot: &Path) -> tempfile::TempDir {
    let copy = tempfile::tempdir().unwrap();
    copy_tree(snapshot, copy.path());
    copy
}

fn parse_file(path: &Path) -> Result<Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }
    sorted_entries(dir)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect()
}

fn stem(path: &Path) -> String {
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

fn id(raw: &str) -> Result<Id, String> {
    Id::parse(raw).map_err(|error| format!("{raw}: {error}"))
}

/// A kept version's file name: `<digits>.json`.
fn version_of(path: &Path) -> Option<u64> {
    let name = file_name(path);
    name.strip_suffix(".json")?.parse().ok()
}

/// The version a `<digits>.recovery-start.json` note belongs to: when this node took a version that starts a
/// recovery.
fn recovery_start_of(path: &Path) -> Option<u64> {
    let name = file_name(path);
    name.strip_suffix(".recovery-start.json")?.parse().ok()
}

/// The versions of one org kept in `orgs/<org>.history/` verify as a chain, and the current definition is
/// the last of them. A node that joined later keeps the chain from the version it joined at, and the first
/// version it kept is checked against its own root key (a snapshot that begins at a version where the root
/// rotated is signed by the previous root and is not covered). Only the names `<digits>.json` and
/// `<digits>.recovery-start.json` are known inside the history, and `orgs/*.json` and `replicas/<org>/` are
/// read as org and team ids: a file of another name there is not read.
fn check_org_chain(store: &dyn OrgStore, data: &Path, org: &Id) -> Result<SignedOrg, String> {
    let history = data.join("orgs").join(format!("{org}.history"));
    let mut chain: Vec<SignedOrg> = Vec::new();
    for path in json_files(&history) {
        if let Some(version) = recovery_start_of(&path) {
            store.recovery_started(org, version).ok_or_else(|| {
                format!("the recovery start of {org} version {version} does not read")
            })?;
            continue;
        }
        let version =
            version_of(&path).ok_or_else(|| format!("{}: not a version number", path.display()))?;
        let doc = store
            .load_org_version(org, version)
            .ok_or_else(|| format!("version {version} of {org} does not read"))?;
        if doc.body.version != version {
            return Err(format!(
                "{} holds version {}",
                path.display(),
                doc.body.version
            ));
        }
        chain.push(doc);
    }
    let first = chain
        .first()
        .ok_or_else(|| format!("{org} has no history"))?;
    if first.body.version == 1 {
        verify_genesis(first).map_err(|error| format!("{org} version 1: {error}"))?;
    } else if first.signer != first.body.root.account
        || !first.verify(Kind::Org, &first.body.root.public_key)
    {
        return Err(format!(
            "the signature of {org} version {} does not match its root key",
            first.body.version
        ));
    }
    for pair in chain.windows(2) {
        verify_successor(&pair[0], &pair[1])
            .map_err(|error| format!("{org} version {}: {error}", pair[1].body.version))?;
    }
    let current = store
        .load_org(org)
        .ok_or_else(|| format!("the current definition of {org} does not read"))?;
    match chain.last() {
        Some(last) if last.digest() == current.digest() => Ok(current),
        _ => Err(format!(
            "the current definition of {org} is not the last version kept"
        )),
    }
}

/// What the node trusts for each org (`trusted` in `config.json`) is the fingerprint of the root key of the
/// org's current definition: a node that takes a rotation moves it to the new key.
fn check_trust(config: &NodeConfig, current: &[SignedOrg]) -> Result<(), String> {
    for doc in current {
        let org = &doc.body.org;
        let trusted = config
            .trusted
            .iter()
            .find(|trusted| &trusted.org == org)
            .ok_or_else(|| format!("the node trusts no root key of {org}"))?;
        if trusted.root_fingerprint != doc.body.root.public_key.fingerprint() {
            return Err(format!(
                "the root key the node trusts for {org} is not the root of its current definition"
            ));
        }
    }
    Ok(())
}

fn check_orgs(store: &dyn OrgStore, data: &Path, config: &NodeConfig) -> Result<(), String> {
    let files: Vec<PathBuf> = json_files(&data.join("orgs"));
    if files.is_empty() {
        return Err("the node holds no org".into());
    }
    let mut current = Vec::new();
    for path in files {
        current.push(check_org_chain(store, data, &id(&stem(&path))?)?);
    }
    check_trust(config, &current)
}

fn check_replicas(store: &dyn ReplicaStore, data: &Path) -> Result<(), String> {
    let replicas = data.join("replicas");
    if !replicas.is_dir() {
        return Ok(());
    }
    for org_dir in sorted_entries(&replicas) {
        for path in json_files(&org_dir) {
            let (org, team) = (id(&file_name(&org_dir))?, id(&stem(&path))?);
            let replica = store
                .load_replica(&org, &team)
                .ok_or_else(|| format!("the replica of {org}/{team} does not read"))?;
            let view = replica
                .view
                .ok_or_else(|| format!("the replica of {org}/{team} holds no view"))?;
            if view.org != org || view.team != team {
                return Err(format!(
                    "the replica of {org}/{team} holds another team's view"
                ));
            }
        }
    }
    Ok(())
}

fn check_decisions(
    store: &(impl DecisionLogStore + AppliedLog),
    data: &Path,
) -> Result<usize, String> {
    let path = data.join("decisions.json");
    if !path.exists() {
        return Ok(0);
    }
    let written = parse_file(&path)?;
    let written = written.as_array().ok_or("decisions.json is not a list")?;
    let read = store.recent_decisions(usize::MAX);
    if read.len() != written.len() {
        return Err(format!(
            "decisions.json holds {} records and the node reads {}",
            written.len(),
            read.len()
        ));
    }
    let applied = parse_file(&data.join("applied.json"))?;
    for id in applied
        .as_object()
        .ok_or("applied.json is not a map")?
        .keys()
    {
        let id: EnvelopeId = serde_json::from_value(Value::String(id.clone()))
            .map_err(|error| format!("applied.json: {error}"))?;
        if !store.is_applied(&id) {
            return Err(format!(
                "applied.json lists {}, which the node does not read",
                id.as_str()
            ));
        }
    }
    Ok(read.len())
}

fn check_invites(store: &dyn InviteStore, data: &Path) -> Result<(), String> {
    for path in json_files(&data.join("invites")) {
        store
            .load_invite(&stem(&path))
            .ok_or_else(|| format!("the invite {} does not read", file_name(&path)))?;
    }
    Ok(())
}

fn check_push_state(store: &dyn PushStore, data: &Path) -> Result<(), String> {
    if data.join("notified.json").exists() && store.notified().is_none() {
        return Err("notified.json does not read".into());
    }
    if data.join("desk-refresh.json").exists() && store.desk_refresh().is_none() {
        return Err("desk-refresh.json does not read".into());
    }
    Ok(())
}

/// The `format` file, when a release wrote one: a whole number that today's release reads (a newer one is
/// refused by the node when it opens, and a file that holds no number says nothing of its release).
fn check_format(data: &Path) -> Result<(), String> {
    let path = data.join("format");
    if !path.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| format!("format: {error}"))?;
    let found: u32 = text
        .trim()
        .parse()
        .map_err(|_| "format does not hold a number".to_owned())?;
    if found > agency_core::data_format::CURRENT {
        return Err(format!(
            "format is {found}, newer than the {} today's node reads",
            agency_core::data_format::CURRENT
        ));
    }
    Ok(())
}

fn check_nothing_unread(data: &Path) -> Result<(), String> {
    let known: BTreeSet<&str> = READ_BELOW.iter().copied().collect();
    for entry in sorted_entries(data) {
        let name = file_name(&entry);
        let is_empty_dir = entry.is_dir() && sorted_entries(&entry).is_empty();
        if !known.contains(name.as_str()) && !is_empty_dir {
            return Err(format!(
                "{name} is in the data directory and has no check in this test"
            ));
        }
    }
    Ok(())
}

/// The day of the snapshot: the time of the latest org version it kept. The node reads it as its clock, so
/// that nothing it reads depends on how long ago the release was.
fn snapshot_time(data: &Path) -> Result<String, String> {
    let mut latest = String::new();
    for path in json_files(&data.join("orgs")) {
        let doc = parse_file(&path)?;
        let at = doc["body"]["at"]
            .as_str()
            .ok_or_else(|| format!("{} has no time", path.display()))?;
        latest = latest.max(at.to_owned());
    }
    Ok(latest)
}

/// Opens today's node on `data` and reads everything in it, through the node's stores and its Client API.
/// Returns how many decisions it read, or what could not be read.
async fn read_everything(data: &Path) -> Result<usize, String> {
    check_nothing_unread(data)?;
    check_format(data)?;
    let store = DataDir::new(data.to_owned());
    let config = store
        .load_config()
        .map_err(|error| format!("config.json: {error}"))?;
    let opened = agency_node::open(
        store.clone(),
        Overrides {
            runtime: Some(Arc::new(MemoryRuntime::default())),
            delivery: Some(Arc::new(MemoryDelivery::default())),
            root_keys: Some(Arc::new(MemoryRootKeys::default())),
            clock: Some(Arc::new(ManualClock::at(&snapshot_time(data)?))),
            ..Overrides::default()
        },
    )
    .map_err(|error| format!("the node does not open: {error:#}"))?;
    if store
        .load_device_key()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Err("the CLI device's key does not read".into());
    }
    check_orgs(&store, data, &config)?;
    check_replicas(&store, data)?;
    check_invites(&store, data)?;
    check_push_state(&store, data)?;
    let decisions = check_decisions(&store, data)?;
    check_client_api(&opened, decisions).await?;
    Ok(decisions)
}

/// What a client of the node reads first, answered from the data just checked.
async fn check_client_api(opened: &agency_node::Opened, decisions: usize) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let app = opened.app(port).map_err(|error| error.to_string())?;
    let server = tokio::spawn(async move {
        let service = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
        axum::serve(listener, service).await.unwrap()
    });
    let result = read_client_api(port, decisions).await;
    server.abort();
    result
}

async fn read_client_api(port: u16, decisions: usize) -> Result<(), String> {
    let get = |path: &'static str| async move {
        let response = reqwest::get(format!("http://127.0.0.1:{port}{path}"))
            .await
            .map_err(|error| format!("{path}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("{path} answers {}", response.status()));
        }
        response
            .json::<Value>()
            .await
            .map_err(|error| format!("{path}: {error}"))
    };
    get("/api/me").await?;
    let log = get("/api/decisions").await?;
    let entries = log["entries"].as_array().map_or(0, Vec::len);
    if entries != decisions {
        return Err(format!(
            "/api/decisions lists {entries} decisions, the files hold {decisions}"
        ));
    }
    Ok(())
}

#[test]
fn a_release_is_kept_to_be_read() {
    let found = snapshots();
    let tags: BTreeSet<&str> = found.iter().map(|(tag, _, _)| tag.as_str()).collect();
    assert!(
        tags.contains("v0.3.0"),
        "the first snapshot, v0.3.0, is missing: {tags:?}"
    );
}

#[tokio::test]
async fn today_s_node_reads_what_each_earlier_release_wrote() {
    for (tag, node, path) in snapshots() {
        let copy = copy_of(&path);
        let decisions = read_everything(copy.path())
            .await
            .unwrap_or_else(|error| panic!("{tag}/{node}: {error}"));
        if node == "alice" {
            assert_eq!(
                decisions, 1,
                "{tag}/alice kept the decision of the sequence"
            );
        }
    }
}

/// The snapshot of the first release, copied, so that a test can break one thing in it.
fn damaged(node: &str, damage: impl FnOnce(&Path)) -> Result<usize, String> {
    let snapshot = snapshots()
        .into_iter()
        .find(|(tag, name, _)| tag == "v0.3.0" && name == node)
        .expect("the v0.3.0 snapshot")
        .2;
    let copy = copy_of(&snapshot);
    damage(copy.path());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(read_everything(copy.path()))
}

fn rewrite(path: &Path, change: impl FnOnce(&mut Value)) {
    let mut value = parse_file(path).unwrap();
    change(&mut value);
    std::fs::write(path, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
}

#[test]
fn an_unchanged_snapshot_passes_the_check_the_damaged_ones_fail() {
    assert_eq!(damaged("alice", |_| {}), Ok(1));
    assert!(damaged("bob", |_| {}).is_ok());
}

#[test]
fn a_changed_signature_fails() {
    let error = damaged("alice", |data| {
        rewrite(&data.join("orgs/acme.history/000002.json"), |doc| {
            let signature = doc["signature"].as_str().unwrap().to_owned();
            let first = if signature.starts_with('A') { 'B' } else { 'A' };
            doc["signature"] = Value::String(format!("{first}{}", &signature[1..]));
        });
    })
    .unwrap_err();
    assert!(
        error.contains("version 2") || error.contains("version 1"),
        "{error}"
    );
}

#[test]
fn a_changed_signature_of_the_first_version_fails() {
    let error = damaged("alice", |data| {
        rewrite(&data.join("orgs/acme.history/000001.json"), |doc| {
            doc["signature"] =
                Value::String(doc["signature"].as_str().unwrap().chars().rev().collect());
        });
    })
    .unwrap_err();
    assert!(
        error.contains("acme version 1: the signature of version 1"),
        "{error}"
    );
}

#[test]
fn a_changed_signature_of_the_version_a_member_joined_at_fails() {
    let error = damaged("bob", |data| {
        rewrite(&data.join("orgs/acme.history/000004.json"), |doc| {
            doc["signature"] =
                Value::String(doc["signature"].as_str().unwrap().chars().rev().collect());
        });
    })
    .unwrap_err();
    assert!(error.contains("does not match its root key"), "{error}");
}

#[test]
fn a_changed_body_fails() {
    let error = damaged("alice", |data| {
        rewrite(&data.join("orgs/acme.history/000003.json"), |doc| {
            doc["body"]["label"] = Value::String("Changed".into());
        });
    })
    .unwrap_err();
    assert!(
        error.contains("version 3") || error.contains("last version"),
        "{error}"
    );
}

#[test]
fn a_gap_in_the_versions_fails() {
    let error = damaged("alice", |data| {
        std::fs::remove_file(data.join("orgs/acme.history/000003.json")).unwrap();
    })
    .unwrap_err();
    assert!(error.contains("version 4"), "{error}");
}

#[test]
fn a_current_definition_that_is_not_the_last_version_fails() {
    let error = damaged("alice", |data| {
        std::fs::copy(
            data.join("orgs/acme.history/000004.json"),
            data.join("orgs/acme.json"),
        )
        .unwrap();
    })
    .unwrap_err();
    assert!(error.contains("not the last version"), "{error}");
}

#[test]
fn a_decision_log_the_node_cannot_read_fails() {
    let error = damaged("alice", |data| {
        rewrite(&data.join("decisions.json"), |log| {
            log.as_array_mut().unwrap()[0]["kind"] = Value::String("no-such-kind".into());
        });
    })
    .unwrap_err();
    assert!(
        error.contains("decisions.json holds 1 records and the node reads 0"),
        "{error}"
    );
}

#[test]
fn a_replica_the_node_cannot_read_fails() {
    let error = damaged("bob", |data| {
        rewrite(&data.join("replicas/acme/app.json"), |replica| {
            replica["view"]["requests"] = Value::String("not a list".into());
        });
    })
    .unwrap_err();
    assert!(
        error.contains("replica of acme/app does not read"),
        "{error}"
    );
}

#[test]
fn a_configuration_the_node_cannot_read_fails() {
    let error = damaged("alice", |data| {
        rewrite(&data.join("config.json"), |config| {
            config["listen"] = Value::Bool(true);
        });
    })
    .unwrap_err();
    assert!(error.contains("config.json"), "{error}");
}

#[test]
fn a_file_with_no_check_fails() {
    let error = damaged("alice", |data| {
        std::fs::write(data.join("something-new.json"), "{}").unwrap();
    })
    .unwrap_err();
    assert!(error.contains("something-new.json"), "{error}");
}

#[test]
fn a_root_key_the_node_does_not_trust_fails() {
    let error = damaged("bob", |data| {
        rewrite(&data.join("config.json"), |config| {
            config["trusted"][0]["rootFingerprint"] = Value::String("0".repeat(32));
        });
    })
    .unwrap_err();
    assert!(
        error.contains("the root key the node trusts for acme"),
        "{error}"
    );
}

#[test]
fn an_org_with_no_trusted_root_key_fails() {
    let error = damaged("bob", |data| {
        rewrite(&data.join("config.json"), |config| {
            config["trusted"] = Value::Array(Vec::new());
        });
    })
    .unwrap_err();
    assert!(error.contains("trusts no root key of acme"), "{error}");
}

#[test]
fn the_note_of_a_recovery_start_is_read_and_is_not_a_version() {
    let note = |data: &Path, text: &str| {
        std::fs::write(
            data.join("orgs/acme.history/000003.recovery-start.json"),
            text,
        )
        .unwrap();
    };
    assert_eq!(
        damaged("alice", |data| note(data, "\"2026-11-24T12:00:00Z\"")),
        Ok(1)
    );
    let error = damaged("alice", |data| note(data, "{}")).unwrap_err();
    assert!(
        error.contains("recovery start of acme version 3 does not read"),
        "{error}"
    );
}

#[test]
fn a_data_format_file_with_a_number_today_s_node_reads_passes() {
    let format = |data: &Path| std::fs::write(data.join("format"), "1\n").unwrap();
    assert_eq!(damaged("alice", format), Ok(1));
}

#[test]
fn a_data_format_file_with_a_newer_number_or_none_fails() {
    let newer = damaged("alice", |data| {
        std::fs::write(data.join("format"), "999\n").unwrap()
    })
    .unwrap_err();
    assert!(newer.contains("newer than"), "{newer}");
    let none = damaged("alice", |data| {
        std::fs::write(data.join("format"), "abc\n").unwrap()
    })
    .unwrap_err();
    assert!(none.contains("format does not hold a number"), "{none}");
}
