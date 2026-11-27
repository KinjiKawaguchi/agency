//! `docs/data.md` names everything a node keeps in its data directory. Every name an adapter writes under it must be
//! in that document: each `.dir("…")` (however rustfmt splits the call), each `self.0.join("…")` or
//! `self.root().join("…")`, and each `.join(NAME)` of a `const NAME: &str = "…";`, in any source of
//! `agency-adapters`, and the few written there by other paths. So a new store cannot land without saying what it
//! holds, for how long, and how to delete it.
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

/// Every Rust source under `dir`, one after another: a store may write from any adapter's own `impl DataDir`.
fn sources(dir: &Path) -> String {
    let mut text = String::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            text.push_str(&sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            text.push_str(&std::fs::read_to_string(&path).unwrap());
        }
    }
    text
}

/// The names joined to a path through a named constant (`.join(FILE)` with `const FILE: &str = "…";`), as a store
/// that keeps its file name in a constant writes it.
fn joined_constants(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let declaration = line
                .trim()
                .trim_start_matches("pub ")
                .strip_prefix("const ")?;
            let (name, value) = declaration.split_once(": &str = \"")?;
            let value = value.strip_suffix("\";")?;
            source
                .contains(&format!(".join({name})"))
                .then(|| value.to_owned())
        })
        .collect()
}

/// What other adapters write directly under the data directory through other paths than the ones scanned.
const ELSEWHERE: [&str; 4] = ["logs/", "metrics/", "usage/", "web-push-vapid.key"];

#[test]
fn every_name_the_store_writes_under_the_data_directory_is_in_the_document() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let store = sources(&root.join("crates/agency-adapters/src"));
    let document = std::fs::read_to_string(root.join("docs/data.md")).unwrap();
    let directories = literals_after(&store, ".dir(\"")
        .into_iter()
        .map(|name| format!("{name}/"));
    let files = literals_after(&store, "self.0.join(\"")
        .into_iter()
        .chain(literals_after(&store, "self.root().join(\""));
    let names: Vec<String> = directories
        .chain(files)
        .chain(joined_constants(&store))
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
