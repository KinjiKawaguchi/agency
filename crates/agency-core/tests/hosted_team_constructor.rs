//! A `HostedTeam` is made with `HostedTeam::new` and its `with_*` methods, never by a struct literal, so a
//! new setting gets its default in one place and not in every place that builds one. Other crates cannot
//! write the literal (the type is `non_exhaustive`); this keeps agency-core from writing it either.
use std::path::{Path, PathBuf};

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

#[test]
fn no_source_builds_a_hosted_team_by_hand() {
    // Spelled in two parts so this file does not match its own needle.
    let needle = format!("{}{}", "HostedTeam", " {");
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = vec![];
    rust_files(&crates, &mut files);
    assert!(files.len() > 50, "the crates' sources were found");
    let mut by_hand = vec![];
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            // The type's own definition, its impls, and a function that returns one are not literals.
            let is_not_literal = [
                "struct HostedTeam",
                "impl HostedTeam",
                "for HostedTeam",
                "-> HostedTeam",
            ]
            .iter()
            .any(|text| line.contains(text));
            if line.contains(&needle) && !is_not_literal {
                by_hand.push(format!("{}:{}", file.display(), number + 1));
            }
        }
    }
    assert!(
        by_hand.is_empty(),
        "build a HostedTeam with HostedTeam::new and its with_ methods: {by_hand:?}"
    );
}
