//! A head of a unit or of the org holds no authority (agency#674): no code that decides who may sign, write, read,
//! join, claim, add a device, decide, or be addressed reads a head. This test reads every source of the crates and
//! fails when a file not listed below names `head`, so that a new reader of a head is added here on purpose, with
//! why, and is read for it.
use std::path::Path;

/// The sources that may name `head`, and why none of them decides authority.
const MAY_NAME_A_HEAD: [(&str, &str); 15] = [
    (
        "crates/agency-proto/src/org.rs",
        "the definition's fields, and the shape-1 checks of a department's head",
    ),
    (
        "crates/agency-proto/src/org_model.rs",
        "the model: who heads a unit and whom one reports to",
    ),
    (
        "crates/agency-proto/src/org_model/tests.rs",
        "the model's tests",
    ),
    (
        "crates/agency-proto/src/org_units.rs",
        "the shape-2 types and their checks",
    ),
    (
        "crates/agency-proto/src/org_units/tests.rs",
        "the shape-2 tests",
    ),
    (
        "crates/agency-proto/src/about.rs",
        "the field guard names the path departments[].head",
    ),
    (
        "crates/agency-proto/src/seat.rs",
        "a test builds a department with no head",
    ),
    (
        "crates/agency-proto/src/directory.rs",
        "the directory a client shows: a department's head, for people to read",
    ),
    (
        "crates/agency-core/src/directory.rs",
        "builds that directory",
    ),
    (
        "crates/agency-core/src/departments.rs",
        "the command that sets a department's head",
    ),
    (
        "crates/agency-core/src/members.rs",
        "says a member heads a department when it is removed",
    ),
    (
        "crates/agency-node/src/dept_cli.rs",
        "prints a department's head",
    ),
    (
        "crates/agency-core/src/delivery/kinds.rs",
        "another meaning: the head of a pull request's title",
    ),
    (
        "crates/agency-node/src/logs_cli.rs",
        "another meaning: `| head` in a comment",
    ),
    (
        "crates/agency-node/src/metrics_cli.rs",
        "another meaning: `| head` in a comment",
    ),
];

/// Whether `source` names a head: the word `head`, not as part of another word such as `header` or `ahead`.
fn names_a_head(source: &str) -> bool {
    let bytes = source.as_bytes();
    source.match_indices("head").any(|(at, _)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + 4).copied();
        let is_word_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

fn sources(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn only_the_listed_sources_name_a_head() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = vec![];
    for krate in [
        "agency-proto",
        "agency-core",
        "agency-adapters",
        "agency-node",
    ] {
        sources(&root.join("crates").join(krate).join("src"), &mut found);
    }
    assert!(found.len() > 100, "the sources are read");
    for path in found {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let is_listed = MAY_NAME_A_HEAD.iter().any(|(file, _)| *file == relative);
        if !is_listed {
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(
                !names_a_head(&source),
                "{relative} names a head. A head holds no authority (#674): if this file must read one, list it in MAY_NAME_A_HEAD with why"
            );
        }
    }
    for (file, _) in MAY_NAME_A_HEAD {
        assert!(
            root.join(file).exists(),
            "{file} is listed and gone: take it out of the list"
        );
    }
}

#[test]
fn the_word_is_found_as_a_word_and_not_inside_another() {
    assert!(names_a_head("unit.head"));
    assert!(names_a_head("org.head()"));
    assert!(names_a_head("head: Some(id)"));
    assert!(!names_a_head("header_text(headers)"));
    assert!(!names_a_head("ahead of time; overhead"));
    assert!(!names_a_head("forehead"));
}
