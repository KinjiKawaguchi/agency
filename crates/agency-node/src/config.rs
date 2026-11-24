//! ノードの設定 (データのディレクトリの config.json)。この台のアカウント、この台の URL、この台に置くチーム、信じる組織。
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Account {
    pub id: Id,
    pub label: String,
}

/// この台に置くチーム (ハーネスの状態のディレクトリ)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostedTeam {
    pub org: Id,
    pub team: Id,
    pub state_dir: PathBuf,
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
    /// ハーネスの CLI (scrum) のパス
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
