//! A head of a unit or of the org holds no authority (agency#674): the code that decides who may sign, write,
//! read, join, claim, add a device or decide never reads a head. This test reads those sources and fails when one
//! of them names `head`, so that authority can't come to depend on a head without this test being changed and
//! read for it.
use std::path::Path;

/// The sources that decide authority, from the workspace root.
const DECIDE_AUTHORITY: [&str; 15] = [
    "crates/agency-core/src/decisions.rs",
    "crates/agency-core/src/signed_decisions.rs",
    "crates/agency-core/src/read_auth.rs",
    "crates/agency-core/src/invites.rs",
    "crates/agency-core/src/devices.rs",
    "crates/agency-core/src/claims.rs",
    "crates/agency-core/src/guests.rs",
    "crates/agency-core/src/kept.rs",
    "crates/agency-core/src/pending_root.rs",
    "crates/agency-adapters/src/client_api/admission.rs",
    "crates/agency-adapters/src/client_api/guests.rs",
    "crates/agency-adapters/src/client_api/claimants.rs",
    "crates/agency-adapters/src/server.rs",
    "crates/agency-proto/src/signed.rs",
    "crates/agency-proto/src/envelope.rs",
];

/// Whether `source` names a head: the word `head` in code (`.head`, `head()`, `head:`), not as part of another
/// word such as `header` or `ahead`.
fn names_a_head(source: &str) -> bool {
    let bytes = source.as_bytes();
    source.match_indices("head").any(|(at, _)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + 4).copied();
        let is_word_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

#[test]
fn no_source_that_decides_authority_reads_a_head() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in DECIDE_AUTHORITY {
        let source = std::fs::read_to_string(root.join(file))
            .unwrap_or_else(|error| panic!("{file}: {error}; update the list if it moved"));
        assert!(
            !names_a_head(&source),
            "{file} names a head. A head holds no authority (#674): if this file must read one, say why here"
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
