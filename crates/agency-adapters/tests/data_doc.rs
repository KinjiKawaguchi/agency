//! `docs/data.md` names everything a node keeps in its data directory. Every name the file store writes under it
//! (each `dir("…")` and `self.0.join("…")` in `files.rs`), and the few other adapters write there, must be in that
//! document, so a new store cannot land without saying what it holds, for how long, and how to delete it.
use std::path::Path;

/// The string literals that follow `pattern` in `source`, such as the `"orgs"` of `self.dir("orgs")`.
fn literals_after(source: &str, pattern: &str) -> Vec<String> {
    source
        .match_indices(pattern)
        .filter_map(|(at, _)| {
            let rest = &source[at + pattern.len()..];
            let end = rest.find('"')?;
            Some(rest[..end].to_owned())
        })
        .collect()
}

/// What other adapters write directly under the data directory, besides the file store.
const ELSEWHERE: [&str; 4] = ["logs/", "metrics/", "usage/", "web-push-vapid.key"];

#[test]
fn every_name_the_store_writes_under_the_data_directory_is_in_the_document() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let store = std::fs::read_to_string(root.join("crates/agency-adapters/src/files.rs")).unwrap();
    let document = std::fs::read_to_string(root.join("docs/data.md")).unwrap();
    let directories = literals_after(&store, "self.dir(\"")
        .into_iter()
        .map(|name| format!("{name}/"));
    let files = literals_after(&store, "self.0.join(\"");
    let names: Vec<String> = directories
        .chain(files)
        .chain(ELSEWHERE.iter().map(|name| (*name).to_owned()))
        .collect();
    assert!(
        names.len() > 20,
        "the scan found the store's paths: {names:?}"
    );
    let missing: Vec<&String> = names
        .iter()
        .filter(|name| !document.contains(&format!("`{name}`")))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/data.md does not say what these hold, for how long, and how to delete them: {missing:?}"
    );
}
