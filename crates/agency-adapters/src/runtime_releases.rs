//! Which harness release each hosted team's runtime says it is (`version` of `describe`, docs/protocol.md). The
//! node reads `describe` once per team and process; what it read is kept in `runtime-releases.json` of the data
//! directory, so that `agency status`, a separate process, can print it. A team with no entry has not been
//! read since the file was made; an entry without a `version` is a runtime that did not say. An entry of a team
//! the node no longer hosts is dropped the next time a team is noted.
use crate::files::{read_json, write_json, DataDir};
use agency_core::ports::Clock;
use agency_proto::clock;
use agency_proto::runtime::Describe;
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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

/// Writes what a runtime said into the file, one entry for each team the node hosts.
///
/// The file is shared by the processes of one node (`agency serve` and the commands, each of which reads
/// `describe` for its own use), so every note reads the file again and adds its entry to what is there; it
/// never writes back the copy it read when it started.
pub struct ReleaseLog {
    path: PathBuf,
    clock: Arc<dyn Clock>,
    /// The teams the node hosts: an entry of any other team is dropped at the next note.
    hosted: BTreeSet<String>,
    /// Serves the notes of this process one at a time.
    writing: Mutex<()>,
}

impl ReleaseLog {
    /// A log in `data`'s directory for the teams `hosted` (org and team).
    pub fn new<'a>(
        data: &DataDir,
        clock: Arc<dyn Clock>,
        hosted: impl IntoIterator<Item = (&'a Id, &'a Id)>,
    ) -> Self {
        Self {
            path: data.root().join(FILE),
            clock,
            hosted: hosted
                .into_iter()
                .map(|(org, team)| key(org, team))
                .collect(),
            writing: Mutex::new(()),
        }
    }

    /// Notes what `description` says about the team's runtime. A file that cannot be written is left
    /// alone: the release is not needed to serve the team.
    pub fn note(&self, org: &Id, team: &Id, description: &Describe) {
        let entry = Reported {
            version: description.release().map(str::to_owned),
            read_at: clock::format(self.clock.now()),
        };
        let _one_at_a_time = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let mut kept: BTreeMap<String, Reported> = read_json(&self.path).unwrap_or_default();
        kept.retain(|name, _| self.hosted.contains(name));
        kept.insert(key(org, team), entry);
        let _ = write_json(&self.path, &kept);
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
