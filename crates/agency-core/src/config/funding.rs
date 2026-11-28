//! The account's sources of funding, and how the node collects the usage of each.
use super::{default_true, is_true};
use agency_proto::client::FundingKind;
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FundingConfig {
    pub id: Id,
    pub label: String,
    pub kind: FundingKind,
    /// A JSON file with the subscription's usage windows (`five_hour`, `seven_day`). Never a token.
    /// Without it, a source that has a collector uses `usage/<id>.json` in the node's data directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_file: Option<PathBuf>,
    /// How the node itself reads this source's usage and writes the usage file. Without it, some
    /// other tool keeps the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collector: Option<UsageCollector>,
    /// New agent sessions leave this source for the next one when its 7-day window passes this many
    /// percent (1 to 100; default 80). The order of `funding` is the order of preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day_threshold: Option<u32>,
}

impl FundingConfig {
    /// The file this source's usage is read from and, with a collector, written to.
    pub fn usage_path(&self, data_dir: &Path) -> Option<PathBuf> {
        match (&self.usage_file, &self.collector) {
            (Some(file), _) => Some(file.clone()),
            (None, Some(_)) => Some(data_dir.join("usage").join(format!("{}.json", self.id))),
            (None, None) => None,
        }
    }
}

/// The program an agent session runs on a funding source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    Claude,
    Codex,
}

impl UsageCollector {
    pub fn tool(&self) -> Tool {
        match self {
            Self::Claude { .. } => Tool::Claude,
            Self::Codex { .. } => Tool::Codex,
        }
    }
}

/// How a funding source's usage is collected. A collector names where the login lives, never a
/// token: the node reads the token itself, each time it needs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum UsageCollector {
    /// A Claude subscription login. `config_dir` is the directory Claude Code runs with as
    /// `CLAUDE_CONFIG_DIR`, written exactly as it is set there (an absolute path); without it, the
    /// default login (`~/.claude`).
    #[serde(rename_all = "camelCase")]
    Claude {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        config_dir: Option<PathBuf>,
        /// Whether the node runs `claude` once, within limits, to renew the login when its access token
        /// has expired and nobody has used the login (default true). It never renews a token itself.
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        renew: bool,
        /// The `claude` program that renews the login (default: `claude` on the node's `PATH`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<PathBuf>,
    },
    /// A ChatGPT plan used through Codex, read from `codex app-server`, which holds the login itself.
    /// `codex_home` is the `CODEX_HOME` the login lives in (default `~/.codex`); `command` is the
    /// `codex` program (default: `codex` on the node's `PATH`).
    #[serde(rename_all = "camelCase")]
    Codex {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        codex_home: Option<PathBuf>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_collector_renews_by_default_and_says_so_in_the_file_only_when_it_does_not() {
        let default: UsageCollector = serde_json::from_str(r#"{"type":"claude"}"#).unwrap();
        assert_eq!(
            default,
            UsageCollector::Claude {
                config_dir: None,
                renew: true,
                command: None
            }
        );
        assert_eq!(
            serde_json::to_string(&default).unwrap(),
            r#"{"type":"claude"}"#,
            "the default is not written"
        );
        let off: UsageCollector = serde_json::from_str(
            r#"{"type":"claude","configDir":"/home/alice/.claude-a","renew":false,"command":"/opt/bin/claude"}"#,
        )
        .unwrap();
        assert_eq!(
            off,
            UsageCollector::Claude {
                config_dir: Some("/home/alice/.claude-a".into()),
                renew: false,
                command: Some("/opt/bin/claude".into())
            }
        );
        assert_eq!(
            serde_json::to_string(&off).unwrap(),
            r#"{"type":"claude","configDir":"/home/alice/.claude-a","renew":false,"command":"/opt/bin/claude"}"#
        );
        assert!(
            serde_json::from_str::<UsageCollector>(r#"{"type":"claude","renews":false}"#).is_err()
        );
        assert!(
            serde_json::from_str::<UsageCollector>(r#"{"type":"codex","renew":false}"#).is_err()
        );
    }
}
