//! ノードの設定 (データのディレクトリの config.json)。この台のアカウント、この台の URL、この台に置くチーム、信じる組織。
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

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
        to.and_then(|name| self.people.get(name)).unwrap_or(&self.owner)
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
        let env = BTreeMap::from([("SCRUM_STATE_DIR".to_owned(), state_dir.display().to_string())]);
        Self { command: vec![scrum.display().to_string(), "runtime".to_owned()], env }
    }
}

/// 信じる組織と、その根の鍵の指紋。ほかの台から届く組織の定義は、ここにある組織のものだけ受け取る
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedOrg {
    pub org: Id,
    pub root_fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeConfig {
    /// この台の頭の URL (ほかの台からの届け先)
    pub url: String,
    /// 待ち受けるアドレス
    pub listen: String,
    pub account: Account,
    /// The scrum harness CLI, used by `team-add --state-dir` to build a team's runtime command.
    pub scrum: PathBuf,
    #[serde(default)]
    pub teams: Vec<HostedTeam>,
    #[serde(default)]
    pub trusted: Vec<TrustedOrg>,
}

impl NodeConfig {
    pub fn hosted(&self, org: &Id, team: &Id) -> Option<&HostedTeam> {
        self.teams.iter().find(|one| &one.org == org && &one.team == team)
    }

    pub fn trusted_fingerprint(&self, org: &Id) -> Option<&str> {
        self.trusted.iter().find(|one| &one.org == org).map(|one| one.root_fingerprint.as_str())
    }

    /// 信じる組織を足す (同じ組織は指紋を置き換える)
    pub fn trust(&mut self, org: Id, root_fingerprint: String) {
        self.trusted.retain(|one| one.org != org);
        self.trusted.push(TrustedOrg { org, root_fingerprint });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn requests_go_to_mapped_people_and_otherwise_to_the_owner() {
        let team = HostedTeam { org: id("acme"), team: id("app"), owner: id("alice"), people: BTreeMap::from([("reviewer".to_owned(), id("bob"))]), runtime: RuntimeCommand { command: vec!["true".into()], env: BTreeMap::new() } };
        assert_eq!(team.addressee(None), &id("alice"));
        assert_eq!(team.addressee(Some("reviewer")), &id("bob"));
        assert_eq!(team.addressee(Some("someone-else")), &id("alice"));
    }
}
