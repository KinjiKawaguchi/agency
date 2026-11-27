//! The `format` file of the data directory: the number of the layout the directory holds (see
//! `agency_core::data_format`). A node reads it before it touches anything else and writes nothing into a
//! directory of a newer layout.
use crate::files::{write_json, DataDir};
use agency_core::data_format::{cannot_read_file, check, not_a_number, CURRENT};
use agency_core::Result;
use std::fs;
use std::path::PathBuf;

impl DataDir {
    fn format_path(&self) -> PathBuf {
        self.root().join("format")
    }

    /// The layout number the directory says it holds, `None` when it has no `format` file.
    fn stored_format(&self) -> Result<Option<u32>> {
        match fs::read_to_string(self.format_path()) {
            Ok(text) => text.trim().parse().map(Some).map_err(|_| not_a_number()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(cannot_read_file()),
        }
    }

    /// Refuses a directory of a newer layout, and writes nothing.
    pub fn check_format(&self) -> Result<()> {
        self.stored_format()?.map_or(Ok(()), check)
    }

    /// Refuses a directory of a newer layout; a directory without a `format` file gets this release's.
    pub fn ensure_format(&self) -> Result<()> {
        match self.stored_format()? {
            Some(found) => check(found),
            None => write_json(&self.format_path(), &CURRENT),
        }
    }
}
