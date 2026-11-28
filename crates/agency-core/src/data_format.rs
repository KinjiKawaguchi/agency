//! The number of the layout this release writes into a node's data directory (the `format` file), and what
//! a node says when it finds data or settings that a newer release wrote. See "Node data and configuration"
//! in docs/compatibility.md.
use crate::error::{Error, ErrorKind, Result};

/// The layout of the data directory this release writes. A directory without a `format` file has this
/// number too, since every earlier release wrote the same layout.
pub const CURRENT: u32 = 1;

/// Refuses data of a layout newer than this release reads. A number below the first (0) is read as the first.
/// The message holds no path and no value from the directory.
pub fn check(found: u32) -> Result<()> {
    if found <= CURRENT {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Conflict,
        format!(
            "this node's data was written by a newer release (data format {found}; this release reads up to {CURRENT}). \
             Update the node to that release, or restore the data from a copy made before the newer release ran. \
             Nothing was changed."
        ),
    ))
}

/// The refusal for a `format` file that holds no number: this node cannot tell which release wrote the data.
pub fn not_a_number() -> Error {
    Error::new(
        ErrorKind::Conflict,
        "the data format file does not hold a number, so this node cannot tell which release wrote the data. \
         Nothing was changed.",
    )
}

/// The refusal for a `format` file that is there and cannot be read (no permission, not a file).
pub fn cannot_read_file() -> Error {
    Error::new(
        ErrorKind::Conflict,
        "the data format file cannot be read, so this node cannot tell which release wrote the data. \
         Nothing was changed.",
    )
}

/// Whether a failure to read `config.json` (the text `serde` gave) is a setting this release does not know:
/// the file is read strictly, so that is also what a setting of a newer release looks like.
pub fn is_unknown_setting(reading_error: &str) -> bool {
    reading_error.starts_with("unknown field") || reading_error.starts_with("unknown variant")
}

/// The refusal for a `config.json` with a setting this release does not know. `reading_error` names the
/// setting; for an unknown variant it also holds the word that was given, which is an enum word and not a
/// value that could be secret.
pub fn unknown_setting(reading_error: &str) -> Error {
    Error::new(
        ErrorKind::Conflict,
        format!(
            "config.json holds a setting this release does not know ({reading_error}). \
             It may come from a newer release: update the node. If the node is up to date, the setting is misspelled."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_of_this_release_and_older_ones_are_read() {
        assert!(check(CURRENT).is_ok());
        assert!(check(0).is_ok());
    }

    #[test]
    fn a_newer_layout_is_refused_with_the_release_to_update_to() {
        let error = check(CURRENT + 1).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Conflict);
        assert!(error.message.contains("newer release"), "{error}");
        assert!(error.message.contains("Update the node"), "{error}");
    }

    #[test]
    fn only_a_word_serde_uses_for_what_it_does_not_know_is_a_newer_setting() {
        assert!(is_unknown_setting(
            "unknown field `fresh`, expected one of `url`"
        ));
        assert!(is_unknown_setting(
            "unknown variant `fresh`, expected `claude`"
        ));
        assert!(!is_unknown_setting(
            "missing field `url` at line 1 column 2"
        ));
        assert!(!is_unknown_setting("expected value at line 1 column 1"));
    }
}
