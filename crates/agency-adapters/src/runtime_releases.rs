//! Which harness release each hosted team's runtime says it is (`version` of `describe`, docs/protocol.md). The
//! node reads `describe` once per team and process; what it read is kept in `runtime-releases.json` of the data
//! directory, so that `agency status`, a separate process, can print it. A team with no entry has not been
//! read since the file was made; an entry without a `version` is a runtime that did not say.
use crate::files::{read_json, write_json, DataDir};
use agency_core::ports::Clock;
use agency_proto::clock;
use agency_proto::runtime::Describe;
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

/// The file of the data directory that holds what the runtimes said.
pub const FILE: &str = "runtime-releases.json";

/// What a team's runtime last said, and when the node read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reported {
    /// The harness release, when the runtime said one that is within the limits (`Describe::release`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub read_at: String,
}

fn key(org: &Id, team: &Id) -> String {
    format!("{org}/{team}")
}

/// What the file holds, by `<org>/<team>`. A file that is not there or cannot be read is nothing.
pub fn read(data: &DataDir) -> BTreeMap<String, Reported> {
    read_json(&data.root().join(FILE)).unwrap_or_default()
}

/// What `status` says of a team's runtime, from what `read` returned.
pub fn say(kept: &BTreeMap<String, Reported>, org: &Id, team: &Id) -> String {
    match kept.get(&key(org, team)) {
        None => "runtime release not read yet".to_owned(),
        Some(Reported {
            version: Some(version),
            read_at,
        }) => format!("runtime release {version}, read {read_at}"),
        Some(Reported {
            version: None,
            read_at,
        }) => format!("runtime does not say its release, read {read_at}"),
    }
}

/// Writes what a runtime said into the file, one entry for each team.
pub struct ReleaseLog {
    path: PathBuf,
    clock: Arc<dyn Clock>,
    kept: Mutex<BTreeMap<String, Reported>>,
}

impl ReleaseLog {
    /// A log in `data`'s directory, which starts from what the file already holds (the entries of other
    /// teams stay until the team is read again).
    pub fn new(data: &DataDir, clock: Arc<dyn Clock>) -> Self {
        Self {
            path: data.root().join(FILE),
            clock,
            kept: Mutex::new(read(data)),
        }
    }

    /// Notes what `description` says about the team's runtime. A file that cannot be written is left
    /// alone: the release is not needed to serve the team.
    pub fn note(&self, org: &Id, team: &Id, description: &Describe) {
        let entry = Reported {
            version: description.release().map(str::to_owned),
            read_at: clock::format(self.clock.now()),
        };
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        kept.insert(key(org, team), entry);
        let _ = write_json(&self.path, &*kept);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn a_team_is_said_by_what_its_runtime_reported() {
        let kept = BTreeMap::from([
            (
                "acme/app".to_owned(),
                Reported {
                    version: Some("0.7.2".into()),
                    read_at: "2026-11-28T01:00:00Z".into(),
                },
            ),
            (
                "acme/old".to_owned(),
                Reported {
                    version: None,
                    read_at: "2026-11-28T01:00:00Z".into(),
                },
            ),
        ]);
        let said = |team| say(&kept, &id("acme"), &id(team));
        assert_eq!(
            said("app"),
            "runtime release 0.7.2, read 2026-11-28T01:00:00Z"
        );
        assert_eq!(
            said("old"),
            "runtime does not say its release, read 2026-11-28T01:00:00Z"
        );
        assert_eq!(said("new"), "runtime release not read yet");
    }
}
