//! `agency export`: an org's records out of a node, in files that verify on their own. The numbers here are
//! made up.
mod common;

use agency_core::{kpis, outcomes};
use agency_node::export::{export_to, verify_path};
use agency_proto::observation::ObservationInput;
use agency_proto::outcome::{GoodWhen, Kpi, KpiKind, KpiUnit};
use agency_proto::Date;
use common::{alice, answer, decide_as, device, id, joined_bob, Running};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn activation() -> Kpi {
    Kpi {
        id: id("activation"),
        label: "Activation".into(),
        kind: KpiKind::Product,
        unit: KpiUnit::Percent,
        good_when: GoodWhen::Higher,
    }
}

/// alice's node with an org of several versions (a member, a KPI), a decision, and a number, the key of the
/// device that signed the decision, and bob's node, which joined the org.
async fn records() -> (Running, agency_proto::AccountKey, Running) {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    kpis::add(&alice.node, &id("acme"), activation()).unwrap();
    let key = device(&alice).await;
    decide_as(&alice, &key, "r-1", answer("yes")).await.unwrap();
    let input = ObservationInput {
        kpi: id("activation"),
        date: Date::parse("2026-11-23").unwrap(),
        value: 41.5,
        note: None,
        connector: None,
    };
    outcomes::record(&alice.node, &id("acme"), input)
        .await
        .unwrap();
    (alice, key, bob)
}

async fn alice_with_records() -> (Running, agency_proto::AccountKey) {
    let (alice, key, _bob) = records().await;
    (alice, key)
}

fn exported(running: &Running) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("export");
    export_to(&running.node, &id("acme"), &to).unwrap();
    (dir, to)
}

fn files_of(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files_of(&path));
        } else {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn error_of(dir: &Path) -> String {
    format!("{:#}", verify_path(dir).unwrap_err())
}

#[tokio::test]
async fn an_export_of_an_org_with_versions_decisions_and_numbers_verifies() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let verified = verify_path(&to).unwrap();
    let last = alice.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(verified.manifest.first_version, 1);
    assert_eq!(verified.manifest.last_version, last.body.version);
    assert!(last.body.version >= 3, "an org with several versions");
    assert_eq!(verified.decisions, 1);
    assert_eq!(verified.numbers, 1);
    let versions = files_of(&to.join("versions"));
    assert_eq!(versions.len() as u64, last.body.version);
}

#[tokio::test]
async fn a_changed_byte_in_any_file_fails_the_verification() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    for path in files_of(&to) {
        if path.ends_with("manifest.json") {
            continue;
        }
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed[original.len() / 2] ^= 1;
        std::fs::write(&path, &changed).unwrap();
        let error = error_of(&to);
        assert!(
            error.contains("is not the file that was exported"),
            "{path:?}: {error}"
        );
        std::fs::write(&path, original).unwrap();
    }
    verify_path(&to).unwrap();
}

/// Replaces a file of the export and lists the new one in the manifest, as someone who changes a file and
/// does not want the manifest to say so would.
fn replace_and_relist(to: &Path, path: &str, text: &str) {
    std::fs::write(to.join(path), text).unwrap();
    let mut manifest = read_json(&to.join("manifest.json"));
    for entry in manifest["files"].as_array_mut().unwrap() {
        if entry["path"] == path {
            entry["sha256"] = Value::String(agency_proto::digest::sha256_hex(text.as_bytes()));
            entry["bytes"] = Value::from(text.len());
        }
    }
    let text = serde_json::to_string_pretty(&manifest).unwrap();
    std::fs::write(to.join("manifest.json"), text).unwrap();
}

#[tokio::test]
async fn a_version_that_is_changed_and_listed_again_fails_as_a_chain() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut doc = read_json(&to.join("versions/000002.json"));
    doc["body"]["label"] = Value::String("Changed".into());
    let text = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    replace_and_relist(&to, "versions/000002.json", &text);
    let error = error_of(&to);
    assert!(
        error.contains("version 2") || error.contains("version 3"),
        "{error}"
    );
}

#[tokio::test]
async fn a_decision_of_another_org_listed_again_fails() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut decisions = read_json(&to.join("decisions.json"));
    decisions[0]["org"] = Value::String("other".into());
    replace_and_relist(&to, "decisions.json", &decisions.to_string());
    assert!(error_of(&to).contains("holds a decision of org other"));
}

#[tokio::test]
async fn a_manifest_that_names_another_org_fails() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut manifest = read_json(&to.join("manifest.json"));
    manifest["org"] = Value::String("other".into());
    std::fs::write(to.join("manifest.json"), manifest.to_string()).unwrap();
    assert!(error_of(&to).contains("versions/000001.json holds version 1 of org acme"));
}

#[tokio::test]
async fn a_file_added_or_taken_away_fails() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    std::fs::write(to.join("notes.txt"), "extra").unwrap();
    assert!(error_of(&to).contains("notes.txt is in the export and not in manifest.json"));
    std::fs::remove_file(to.join("notes.txt")).unwrap();
    std::fs::remove_file(to.join("decisions.json")).unwrap();
    assert!(error_of(&to).contains("decisions.json is listed and is not in the export"));
}

#[tokio::test]
async fn an_export_of_a_newer_format_is_refused() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut manifest = read_json(&to.join("manifest.json"));
    manifest["format"] = Value::from(2);
    std::fs::write(to.join("manifest.json"), manifest.to_string()).unwrap();
    assert!(error_of(&to).contains("made by a newer node"));
}

#[tokio::test]
async fn a_manifest_that_names_another_root_key_fails() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut manifest = read_json(&to.join("manifest.json"));
    manifest["rootFingerprint"] = Value::String("0".repeat(32));
    std::fs::write(to.join("manifest.json"), manifest.to_string()).unwrap();
    assert!(error_of(&to).contains("root key"));
}

#[tokio::test]
async fn no_file_holds_a_key() {
    let (alice, device_key) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let account_key = std::fs::read_to_string(alice.data_dir().join("account.key")).unwrap();
    let secrets = [account_key.trim().to_owned(), device_key.encoded()];
    for path in files_of(&to) {
        let text = std::fs::read_to_string(&path).unwrap();
        for secret in &secrets {
            assert!(
                !text.contains(secret.as_str()),
                "{path:?} holds a private key"
            );
        }
    }
}

#[tokio::test]
async fn a_directory_that_holds_something_is_refused_and_left_as_it_is() {
    let (alice, _) = alice_with_records().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("keep.txt"), "mine").unwrap();
    let error = export_to(&alice.node, &id("acme"), dir.path()).unwrap_err();
    assert!(format!("{error:#}").contains("is not empty"), "{error:#}");
    assert_eq!(files_of(dir.path()), vec![dir.path().join("keep.txt")]);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("keep.txt")).unwrap(),
        "mine"
    );
}

#[tokio::test]
async fn an_empty_directory_is_filled() {
    let (alice, _) = alice_with_records().await;
    let dir = tempfile::tempdir().unwrap();
    export_to(&alice.node, &id("acme"), dir.path()).unwrap();
    verify_path(dir.path()).unwrap();
}

#[tokio::test]
async fn a_member_node_exports_what_it_kept_and_holds_no_numbers() {
    let (_alice, _, bob) = records().await;
    let (_dir, to) = exported(&bob);
    let verified = verify_path(&to).unwrap();
    assert!(
        verified.manifest.first_version > 1,
        "a joined node keeps the versions from its join"
    );
    assert_eq!(verified.numbers, 0);
    assert!(!to.join("outcomes.json").exists());
}

#[tokio::test]
async fn an_org_the_node_does_not_hold_is_refused() {
    let (alice, _) = alice().await;
    let dir = tempfile::tempdir().unwrap();
    let error = export_to(&alice.node, &id("nowhere"), &dir.path().join("out")).unwrap_err();
    assert!(error.to_string().contains("holds no org"), "{error}");
    assert!(!dir.path().join("out").exists());
}

#[tokio::test]
async fn another_orgs_decisions_are_not_in_the_export() {
    let (alice, _) = alice_with_records().await;
    let other = agency_core::decisions::DecisionRecord {
        id: agency_proto::EnvelopeId::from_bytes([9; 16]),
        org: id("other"),
        team: id("app"),
        kind: agency_proto::EnvelopeKind::Answer,
        target: "r-9".into(),
        title: None,
        choice_label: None,
        relayed_for: None,
        payload: answer("no"),
        at: "2026-11-24T12:00:00Z".into(),
        is_local: true,
        by: None,
        signer: None,
    };
    alice.node.store().append_decision(&other).unwrap();
    let (_dir, to) = exported(&alice);
    assert_eq!(verify_path(&to).unwrap().decisions, 1);
}

#[tokio::test]
async fn a_history_with_a_version_missing_is_not_exported() {
    let (alice, _) = alice_with_records().await;
    std::fs::remove_file(alice.data_dir().join("orgs/acme.history/000002.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let error = export_to(&alice.node, &id("acme"), &dir.path().join("out")).unwrap_err();
    assert!(error.to_string().contains("a version missing"), "{error}");
    assert!(!dir.path().join("out").exists());
}

#[tokio::test]
async fn a_size_in_the_manifest_that_is_not_the_files_fails() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut manifest = read_json(&to.join("manifest.json"));
    manifest["files"][0]["bytes"] = Value::from(1);
    std::fs::write(to.join("manifest.json"), manifest.to_string()).unwrap();
    assert!(error_of(&to).contains("is not the file that was exported"));
}

#[tokio::test]
async fn a_current_definition_that_is_not_the_last_version_kept_is_not_exported() {
    let (alice, _) = alice_with_records().await;
    let last = alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version;
    std::fs::remove_file(
        alice
            .data_dir()
            .join(format!("orgs/acme.history/{last:06}.json")),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let error = export_to(&alice.node, &id("acme"), &dir.path().join("out")).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("not the last version this node kept"),
        "{error}"
    );
}

#[tokio::test]
async fn a_file_listed_twice_fails() {
    let (alice, _) = alice_with_records().await;
    let (_dir, to) = exported(&alice);
    let mut manifest = read_json(&to.join("manifest.json"));
    let first = manifest["files"][0].clone();
    manifest["files"].as_array_mut().unwrap().push(first);
    std::fs::write(to.join("manifest.json"), manifest.to_string()).unwrap();
    assert!(error_of(&to).contains("lists versions/000001.json twice"));
}
