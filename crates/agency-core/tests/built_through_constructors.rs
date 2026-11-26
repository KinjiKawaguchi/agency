//! Config and definition structs are built through their constructor, never by a struct literal: a new
//! field then gets its default in one place and not in every place that builds one. Other crates cannot
//! write the literal (the types are `non_exhaustive`); this keeps agency-core from writing it either, and
//! catches a literal anywhere in the workspace's sources.
use std::path::{Path, PathBuf};

/// The types built through `Type::new` and its `with_*` methods.
const CONSTRUCTED: [&str; 2] = ["HostedTeam", "TeamPlace"];

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                rust_files(&path, found);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// A struct literal of `name`: `Name {` that is not its definition, one of its impls, or a function that
/// returns one.
fn builds_by_hand(line: &str, name: &str) -> bool {
    // Spelled in two parts so this file does not match its own needle.
    let needle = format!("{name}{}", " {");
    let is_not_literal = [
        format!("struct {name}"),
        format!("impl {name}"),
        format!("for {name}"),
        format!("-> {name}"),
    ]
    .iter()
    .any(|text| line.contains(text));
    line.contains(&needle) && !is_not_literal
}

#[test]
fn no_source_builds_a_constructed_type_by_hand() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = vec![];
    rust_files(&crates, &mut files);
    assert!(files.len() > 50, "the crates' sources were found");
    let mut by_hand = vec![];
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            for name in CONSTRUCTED {
                if builds_by_hand(line, name) {
                    by_hand.push(format!("{name} at {}:{}", file.display(), number + 1));
                }
            }
        }
    }
    assert!(
        by_hand.is_empty(),
        "build these with Type::new and its with_ methods: {by_hand:?}"
    );
}
