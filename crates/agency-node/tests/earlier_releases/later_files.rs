//! The entries of a data directory that releases after v0.3.0 write (`own-origins.json`, `push-retries.json`,
//! `runtime-releases.json`, `about/`, and `format`, which opening the node reads): what the node reads of each is
//! compared with what is on disk, field by field, so that a reader that returns something else of the same size
//! fails. What a snapshot cannot hold (a retry, a runtime release that says its version) is written by hand as
//! the release's types write it.
use super::{damaged_in, id, json_files, parse_file, rewrite, stem};
use agency_adapters::files::DataDir;
use agency_core::ports::{AboutStore, OwnOriginStore, PushStore};
use agency_core::push_retries::Destination;
use serde_json::Value;
use std::path::Path;

/// Reads each of these entries through the node, when the data directory has it.
pub(super) fn check(store: &DataDir, data: &Path) -> Result<(), String> {
    check_own_origins(store, data)?;
    check_push_retries(store, data)?;
    check_runtime_releases(store, data)?;
    check_peer_abouts(store, data)
}

/// `own-origins.json`: the origins the node reads are those on disk, in the same order.
fn check_own_origins(store: &dyn OwnOriginStore, data: &Path) -> Result<(), String> {
    let path = data.join("own-origins.json");
    if !path.exists() {
        return Ok(());
    }
    let written = parse_file(&path)?;
    let written = written.as_array().ok_or("own-origins.json is not a list")?;
    let read = store
        .own_origins()
        .map_err(|error| format!("own-origins.json: {error}"))?;
    let read: Vec<Value> = read.into_iter().map(Value::String).collect();
    if &read != written {
        return Err(format!(
            "own-origins.json holds {written:?} and the node reads {read:?}"
        ));
    }
    Ok(())
}

/// `push-retries.json`: each retry the node reads is the one on disk, field for field.
fn check_push_retries(store: &dyn PushStore, data: &Path) -> Result<(), String> {
    let path = data.join("push-retries.json");
    if !path.exists() {
        return Ok(());
    }
    let written = parse_file(&path)?;
    let written = written
        .as_array()
        .ok_or("push-retries.json is not a list")?;
    let read: Vec<Value> = store
        .push_retries()
        .iter()
        .map(|retry| serde_json::to_value(retry).unwrap())
        .collect();
    if &read != written {
        return Err(format!(
            "push-retries.json holds {} retries and the node reads {}, or reads them differently",
            written.len(),
            read.len()
        ));
    }
    Ok(())
}

/// `runtime-releases.json`: for each team the node reads the time and the release the file holds.
fn check_runtime_releases(store: &DataDir, data: &Path) -> Result<(), String> {
    let path = data.join("runtime-releases.json");
    if !path.exists() {
        return Ok(());
    }
    let written = parse_file(&path)?;
    let written = written
        .as_object()
        .ok_or("runtime-releases.json is not a map")?;
    let read = agency_adapters::runtime_releases::read(store);
    if read.len() != written.len() {
        return Err(format!(
            "runtime-releases.json holds {} teams and the node reads {}",
            written.len(),
            read.len()
        ));
    }
    for (team, entry) in written {
        let kept = read
            .get(team)
            .ok_or_else(|| format!("runtime-releases.json: the node does not read {team}"))?;
        let version = entry.get("version").and_then(Value::as_str);
        if kept.read_at != entry["readAt"].as_str().unwrap_or_default()
            || kept.version.as_deref() != version
        {
            return Err(format!(
                "runtime-releases.json: {team} is read as {kept:?}, the file holds {entry}"
            ));
        }
    }
    Ok(())
}

/// How many entries a list or a map holds.
fn len_of(value: &Value) -> usize {
    value
        .as_array()
        .map(Vec::len)
        .or_else(|| value.as_object().map(serde_json::Map::len))
        .unwrap_or(0)
}

/// `about/<org>.json`: for each node the node reads what the file holds: what it answered (its release and the
/// number of its capabilities), when it was read and when it was tried.
fn check_peer_abouts(store: &dyn AboutStore, data: &Path) -> Result<(), String> {
    for path in json_files(&data.join("about")) {
        let org = id(&stem(&path))?;
        let written = parse_file(&path)?;
        let written = written
            .as_object()
            .ok_or_else(|| format!("about/{org}.json is not a map"))?;
        let read = store.peer_abouts(&org);
        if read.len() != written.len() {
            return Err(format!(
                "about/{org}.json holds {} nodes and the node reads {}",
                written.len(),
                read.len()
            ));
        }
        for (origin, entry) in written {
            let kept = read
                .get(origin)
                .ok_or_else(|| format!("about/{org}.json: the node does not read {origin}"))?;
            let kept = serde_json::to_value(kept).unwrap();
            let matches = kept["readAt"] == entry["readAt"]
                && kept["triedAt"] == entry["triedAt"]
                && kept["about"]["node"] == entry["about"]["node"]
                && len_of(&kept["about"]["capabilities"])
                    == len_of(&entry["about"]["capabilities"]);
            if !matches {
                return Err(format!(
                    "about/{org}.json: {origin} is read as {kept}, the file holds {entry}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn a_layout_newer_than_the_node_fails() {
    let error = damaged_in("v0.4.0", "alice", |data| {
        std::fs::write(data.join("format"), "99\n").unwrap();
    })
    .unwrap_err();
    assert!(
        error.contains("written by a newer release (data format 99"),
        "{error}"
    );
}

#[test]
fn own_origins_the_node_cannot_read_fail() {
    let error = damaged_in("v0.4.0", "alice", |data| {
        std::fs::write(data.join("own-origins.json"), "{\"not\": \"a list\"}").unwrap();
    })
    .unwrap_err();
    assert!(error.contains("own-origins.json"), "{error}");
}

#[test]
fn push_retries_the_node_cannot_read_fail() {
    let error = damaged_in("v0.4.0", "alice", |data| {
        std::fs::write(data.join("push-retries.json"), "[{\"key\": 1}]").unwrap();
    })
    .unwrap_err();
    assert!(error.contains("push-retries.json holds 1"), "{error}");
}

#[test]
fn a_runtime_release_the_node_cannot_read_fails() {
    let error = damaged_in("v0.4.0", "alice", |data| {
        rewrite(&data.join("runtime-releases.json"), |kept| {
            kept["acme/app"]["readAt"] = Value::from(5);
        });
    })
    .unwrap_err();
    assert!(error.contains("runtime-releases.json holds 1"), "{error}");
}

#[test]
fn what_a_node_said_about_itself_that_cannot_be_read_fails() {
    let error = damaged_in("v0.4.0", "bob", |data| {
        rewrite(&data.join("about/acme.json"), |abouts| {
            let origin = abouts.as_object().unwrap().keys().next().unwrap().clone();
            abouts[origin.as_str()] = Value::String("not an about".into());
        });
    })
    .unwrap_err();
    assert!(error.contains("about/acme.json holds 1"), "{error}");
}

/// A retry as v0.4.0's type writes it (`Retry` and `Destination` of `push_retries.rs` at that tag; the snapshot's
/// run has no push destination that refuses, so its file is empty): written by hand, read through the node.
const V0_4_0_RETRY: &str = r#"[{
  "key": "acme/app/request-1",
  "destination": { "kind": "token", "id": "device-token-1" },
  "since": "2026-11-28T08:00:00Z"
}]"#;

#[test]
fn a_retry_v0_4_0_wrote_is_read() {
    let result = damaged_in("v0.4.0", "alice", |data| {
        std::fs::write(data.join("push-retries.json"), V0_4_0_RETRY).unwrap();
        let retries = DataDir::new(data.to_owned()).push_retries();
        assert_eq!(retries.len(), 1);
        assert_eq!(retries[0].key, "acme/app/request-1");
        assert_eq!(
            retries[0].destination,
            Destination::Token("device-token-1".into())
        );
        assert_eq!(retries[0].since, "2026-11-28T08:00:00Z");
    });
    assert_eq!(result, Ok(1));
}

#[test]
fn a_retry_with_a_field_the_node_does_not_know_is_not_read() {
    let error = damaged_in("v0.4.0", "alice", |data| {
        let hand_written = V0_4_0_RETRY.replace("\"since\"", "\"first\"");
        std::fs::write(data.join("push-retries.json"), hand_written).unwrap();
    })
    .unwrap_err();
    assert!(error.contains("push-retries.json holds 1"), "{error}");
}

/// The snapshot's fixture runtime does not say its release, so the entry that does is written by hand, as v0.4.0
/// writes it (`Reported` of `runtime_releases.rs` at that tag).
#[test]
fn a_runtime_release_v0_4_0_wrote_with_its_version_is_read() {
    let result = damaged_in("v0.4.0", "alice", |data| {
        rewrite(&data.join("runtime-releases.json"), |kept| {
            kept["acme/app"]["version"] = Value::String("2.1.0".into());
        });
    });
    assert_eq!(result, Ok(1));
}
