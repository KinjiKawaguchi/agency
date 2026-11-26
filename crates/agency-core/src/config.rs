//! The node's configuration (`config.json` in its data directory): its account, its URL, the teams it hosts, and the orgs it trusts.
use agency_proto::client::FundingKind;
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Account {
    pub id: Id,
    pub label: String,
}

/// A team this node hosts, and the command that runs its agency-runtime/1 runtime.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostedTeam {
    pub org: Id,
    pub team: Id,
    /// The org person who owns the team. Requests without an addressee go to them.
    pub owner: Id,
    /// Runtime names of other addressees, mapped to org person ids.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub people: BTreeMap<String, Id>,
    pub runtime: RuntimeCommand,
}

impl HostedTeam {
    /// The org person a request is addressed to: `to` from the `people` map, otherwise the owner.
    pub fn addressee(&self, to: Option<&str>) -> &Id {
        to.and_then(|name| self.people.get(name))
            .unwrap_or(&self.owner)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCommand {
    /// The program and its arguments.
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

impl RuntimeCommand {
    /// The scrum harness's runtime for the team whose state lives in `state_dir`.
    pub fn scrum(scrum: &std::path::Path, state_dir: &std::path::Path) -> Self {
        let env = BTreeMap::from([(
            "SCRUM_STATE_DIR".to_owned(),
            state_dir.display().to_string(),
        )]);
        Self {
            command: vec![scrum.display().to_string(), "runtime".to_owned()],
            env,
        }
    }
}

/// A trusted org and its root key's fingerprint. A first copy of an org definition from another node is accepted only for these orgs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedOrg {
    pub org: Id,
    pub root_fingerprint: String,
    /// The origin of the node that holds the root key, where device changes go. Recorded when joining
    /// through an invite, which only the root's node creates; absent for orgs this node is root of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_node: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeConfig {
    /// This node's origin, where other nodes send to.
    pub url: String,
    /// The address to listen on.
    pub listen: String,
    pub account: Account,
    /// The scrum harness CLI, used by `team-add --state-dir` to build a team's runtime command.
    pub scrum: PathBuf,
    #[serde(default)]
    pub teams: Vec<HostedTeam>,
    #[serde(default)]
    pub trusted: Vec<TrustedOrg>,
    /// The account's sources of funding, shown to clients with their usage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding: Vec<FundingConfig>,
    /// Cloudflare Access in front of the Client API. Without it, the Client API answers only direct
    /// local requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessConfig>,
    /// Apple Push Notification service, for the owner's iPhone. Without it, nothing is sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<PushConfig>,
}

/// The APNs provider key (token-based authentication) and the app it sends to. The key file stays
/// outside the repository and is never copied into the data directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushConfig {
    /// The `.p8` file Apple issued for the key.
    pub key_path: PathBuf,
    /// The key's id (10 characters).
    pub key_id: String,
    /// The Apple developer team id (10 characters).
    pub team_id: String,
    /// The app's bundle id, the APNs topic.
    pub bundle_id: String,
}

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

/// Cloudflare Access for requests that arrive through a tunnel or at the public host name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessConfig {
    /// The public host name the tunnel serves, such as `agency.example.com`.
    pub public_host: String,
    /// The Zero Trust team domain, such as `acme.cloudflareaccess.com`.
    pub team_domain: String,
    /// The Access application's audience tag (64 hex characters).
    pub audience: String,
    /// Only this signed-in email may write: writes act for the node's account (they relay its
    /// decisions and device changes).
    pub owner_email: String,
    /// The iOS app that may receive the Access session after a sign-in outside its web view,
    /// `<team id>.<bundle id>`. Without it (or with a malformed one) the node has no handoff route and
    /// no app association file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
}

impl NodeConfig {
    pub fn hosted(&self, org: &Id, team: &Id) -> Option<&HostedTeam> {
        self.teams
            .iter()
            .find(|one| &one.org == org && &one.team == team)
    }

    pub fn trusted_fingerprint(&self, org: &Id) -> Option<&str> {
        self.trusted
            .iter()
            .find(|one| &one.org == org)
            .map(|one| one.root_fingerprint.as_str())
    }

    /// Trusts `root_fingerprint` as the root of `org`. An org id names one org on a node, so this never
    /// replaces the fingerprint of another root; callers refuse that case first (see
    /// `invites::join` and `orgs::create`).
    pub fn trust(&mut self, org: Id, root_fingerprint: String) {
        if self.trusted.iter().any(|one| one.org == org) {
            return;
        }
        self.trusted.push(TrustedOrg {
            org,
            root_fingerprint,
            root_node: None,
        });
    }

    /// `trust`, and where the root's node is. It records the root's node only for the same root.
    pub fn trust_with_root_node(&mut self, org: Id, root_fingerprint: String, root_node: String) {
        self.trust(org.clone(), root_fingerprint.clone());
        let same = self
            .trusted
            .iter_mut()
            .find(|one| one.org == org && one.root_fingerprint == root_fingerprint);
        if let Some(one) = same {
            one.root_node = Some(root_node);
        }
    }

    pub fn root_node(&self, org: &Id) -> Option<&str> {
        self.trusted
            .iter()
            .find(|one| &one.org == org)
            .and_then(|one| one.root_node.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn trusting_never_replaces_another_roots_fingerprint() {
        let mut config = NodeConfig {
            url: "http://127.0.0.1:4400".into(),
            listen: "127.0.0.1:4400".into(),
            account: Account {
                id: id("alice"),
                label: "Alice".into(),
            },
            scrum: "scrum".into(),
            teams: vec![],
            trusted: vec![],
            funding: vec![],
            access: None,
            push: None,
        };
        let acme = id("acme");
        config.trust_with_root_node(acme.clone(), "a".repeat(32), "https://a.example".into());
        config.trust(acme.clone(), "b".repeat(32));
        config.trust_with_root_node(acme.clone(), "b".repeat(32), "https://b.example".into());
        assert_eq!(
            config.trusted_fingerprint(&acme),
            Some("a".repeat(32).as_str())
        );
        assert_eq!(config.root_node(&acme), Some("https://a.example"));
        config.trust_with_root_node(acme.clone(), "a".repeat(32), "https://moved.example".into());
        assert_eq!(
            config.root_node(&acme),
            Some("https://moved.example"),
            "the same root may move"
        );
        assert_eq!(config.trusted.len(), 1);
    }

    #[test]
    fn requests_go_to_mapped_people_and_otherwise_to_the_owner() {
        let team = HostedTeam {
            org: id("acme"),
            team: id("app"),
            owner: id("alice"),
            people: BTreeMap::from([("reviewer".to_owned(), id("bob"))]),
            runtime: RuntimeCommand {
                command: vec!["true".into()],
                env: BTreeMap::new(),
            },
        };
        assert_eq!(team.addressee(None), &id("alice"));
        assert_eq!(team.addressee(Some("reviewer")), &id("bob"));
        assert_eq!(team.addressee(Some("someone-else")), &id("alice"));
    }
}
