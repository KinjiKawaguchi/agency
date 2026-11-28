//! The manifest of an export: what each file is, so that a change to any of them is seen.
use agency_proto::digest::sha256_hex;
use agency_proto::Id;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// The layout of an export. A verifier refuses a higher number: it was made by a newer node, and what it
/// would check is not all there is.
pub const FORMAT: u64 = 1;

/// Where the manifest is, in the export's directory. It lists every other file and is not listed itself.
pub const MANIFEST_PATH: &str = "manifest.json";

/// One file of an export: its path under the export's directory (always with `/`) and its content.
pub struct ExportFile {
    pub path: String,
    pub content: Vec<u8>,
}

impl ExportFile {
    /// A file that holds `value` as indented JSON with a final line break.
    pub fn json<T: Serialize + ?Sized>(path: impl Into<String>, value: &T) -> Result<Self> {
        let text = serde_json::to_string_pretty(value)? + "\n";
        Ok(Self {
            path: path.into(),
            content: text.into_bytes(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub format: u64,
    pub org: Id,
    /// The release of the node that made the export.
    pub release: String,
    pub exported_at: String,
    /// The fingerprint of the root key of the org's last version kept here. The manifest is not signed, so a
    /// reader compares it with the root key they know the org by.
    pub root_fingerprint: String,
    pub first_version: u64,
    pub last_version: u64,
    pub files: Vec<ManifestFile>,
}

/// The facts of an export that its files do not say, for `Manifest::describing`.
pub struct Facts {
    pub org: Id,
    pub release: String,
    pub exported_at: String,
    pub root_fingerprint: String,
    pub first_version: u64,
    pub last_version: u64,
}

impl Manifest {
    /// The manifest of `files`: each one's SHA-256 and size.
    pub fn describing(facts: Facts, files: &[ExportFile]) -> Self {
        Self {
            format: FORMAT,
            org: facts.org,
            release: facts.release,
            exported_at: facts.exported_at,
            root_fingerprint: facts.root_fingerprint,
            first_version: facts.first_version,
            last_version: facts.last_version,
            files: files
                .iter()
                .map(|file| ManifestFile {
                    path: file.path.clone(),
                    sha256: sha256_hex(&file.content),
                    bytes: file.content.len() as u64,
                })
                .collect(),
        }
    }
}
