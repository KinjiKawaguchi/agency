//! Config and definition structs are built through their constructor, never by a struct literal: a new
//! field then gets its default in one place and not in every place that builds one. Other crates cannot
//! write the literal (the types are `non_exhaustive`); this keeps agency-core from writing it either, and
//! catches a literal anywhere in the workspace's sources.
use std::path::{Path, PathBuf};

/// The types built through `Type::new` and its `with_*` methods.
const CONSTRUCTED: [&str; 5] = [
    "HostedTeam",
    "NodeConfig",
    "OrgDefinition",
    "Person",
    "TeamPlace",
];

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

/// Whether `line` holds `text` where the name in it starts a word, so that `HomeNodeConfig`, another
/// type, is not taken for `NodeConfig`; and not as the variant of an enum (`WaitingOn::Person {`), which is a
/// path whose last segment before the name starts with a capital.
fn contains_word(line: &str, text: &str) -> bool {
    line.match_indices(text).any(|(at, _)| {
        let head = &line[..at];
        let before = head.chars().next_back();
        let is_variant = head.strip_suffix("::").is_some_and(|path| {
            let segment = path
                .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next();
            segment.is_some_and(|one| one.starts_with(|c: char| c.is_ascii_uppercase()))
        });
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && !is_variant
    })
}

/// A struct literal of `name`: `Name {` that is not its definition, one of its impls, or a function that
/// returns one. Only the whole name counts, so a type whose name ends with it is not taken for it.
fn builds_by_hand(line: &str, name: &str) -> bool {
    // Spelled in two parts so this file does not match its own needle.
    let needle = format!("{name}{}", " {");
    let is_not_literal = [
        format!("struct {name}"),
        format!("impl {name}"),
        format!("for {name}"),
    ]
    .iter()
    .any(|text| line.contains(text));
    // A return type, with or without a path before the name (`-> agency_proto::Name {`), opens a body.
    let is_return_type = line
        .find("->")
        .zip(line.find(&needle))
        .is_some_and(|(arrow, at)| arrow < at);
    // A variant of an enum declared with a constructed type's name, one field to a line: its fields are types.
    let is_variant_declaration = line.trim_start().starts_with(&needle)
        && line.trim_end().ends_with("},")
        && line.contains(": String");
    contains_word(line, &needle) && !is_not_literal && !is_return_type && !is_variant_declaration
}

#[test]
fn an_enum_variant_of_the_same_name_is_not_a_literal() {
    let variant = format!("Some(WaitingOn::Person{}", " { person })");
    assert!(!builds_by_hand(&variant, "Person"));
    let declared = format!("    Person{}", " { person: String },");
    assert!(!builds_by_hand(&declared, "Person"));
    let qualified = format!("let one = agency_proto::org::Person{}", " {");
    assert!(
        builds_by_hand(&qualified, "Person"),
        "a module path is still a literal"
    );
}

#[test]
fn only_the_whole_name_is_taken_for_a_literal() {
    let literal = format!("let config = NodeConfig{}", " {");
    let other = format!("let entry = HomeNodeConfig{}", " {");
    let qualified = format!("agency_core::config::NodeConfig{}", " {");
    let returned = format!("fn body() -> agency_proto::NodeConfig{}", " {");
    assert!(builds_by_hand(&literal, "NodeConfig"));
    assert!(
        builds_by_hand(&qualified, "NodeConfig"),
        "a path before the name still counts"
    );
    assert!(
        !builds_by_hand(&returned, "NodeConfig"),
        "a function that returns it, by a path, is not a literal"
    );
    assert!(
        !builds_by_hand(&other, "NodeConfig"),
        "another type that ends with the name"
    );
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
