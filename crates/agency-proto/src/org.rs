//! 組織の定義。組織の根の鍵だけが署名でき、版は 1 ずつ増え、前の版の指紋でつながる (書き手は 1 つ)。
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::signed::{Kind, Signed};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Root {
    pub account: Id,
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Department {
    pub id: Id,
    pub label: String,
    pub parent: Option<Id>,
    /// The head of the department: a person id.
    pub head: Option<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Role {
    /// Reads and answers.
    Member,
    /// Only answers.
    Answerer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Person {
    pub id: Id,
    pub label: String,
    pub account: Id,
    pub public_key: PublicKey,
    pub department: Option<Id>,
    pub role: Role,
}

/// A team and the node it runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TeamPlace {
    pub id: Id,
    pub label: String,
    pub department: Option<Id>,
    /// The node's origin (http or https, no path).
    pub node: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OrgDefinition {
    pub org: Id,
    pub label: String,
    #[ts(type = "number")]
    pub version: u64,
    pub prev: Option<String>,
    pub root: Root,
    #[serde(default)]
    pub departments: Vec<Department>,
    #[serde(default)]
    pub people: Vec<Person>,
    #[serde(default)]
    pub teams: Vec<TeamPlace>,
    #[serde(default)]
    pub revoked: Vec<PublicKey>,
    pub at: String,
}

pub type SignedOrg = Signed<OrgDefinition>;

/// 台の頭の URL か (http か https で、ホストとポートだけ)
pub fn is_node_origin(raw: &str) -> bool {
    let rest = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"));
    rest.is_some_and(|host| {
        !host.is_empty()
            && host.len() <= 253
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
    })
}

fn label_ok(label: &str) -> bool {
    let count = label.chars().count();
    (1..=80).contains(&count) && !label.chars().any(char::is_control)
}

fn unique<'a>(ids: impl Iterator<Item = &'a Id>) -> bool {
    let mut seen = HashSet::new();
    ids.into_iter().all(|id| seen.insert(id))
}

impl OrgDefinition {
    /// 所属する人: 組織の定義にあり、鍵が取り消されていない
    pub fn member(&self, account: &Id) -> Option<&Person> {
        self.people
            .iter()
            .find(|one| &one.account == account && !self.revoked.contains(&one.public_key))
    }

    pub fn team(&self, id: &Id) -> Option<&TeamPlace> {
        self.teams.iter().find(|one| &one.id == id)
    }

    /// 形の確かめ: 名前の長さ、id の重なり、部署と長と人と置き場所のつながり
    pub fn validate(&self) -> Result<(), String> {
        if !label_ok(&self.label) || self.version == 0 || (self.version == 1) != self.prev.is_none()
        {
            return Err("組織の名前か版の形が違う".into());
        }
        if !unique(self.departments.iter().map(|d| &d.id))
            || !unique(self.people.iter().map(|p| &p.id))
            || !unique(self.teams.iter().map(|t| &t.id))
            || !unique(self.people.iter().map(|p| &p.account))
        {
            return Err("部署、人、チーム、アカウントのどれかが重なっている".into());
        }
        let departments: HashSet<_> = self.departments.iter().map(|d| &d.id).collect();
        let people: HashSet<_> = self.people.iter().map(|p| &p.id).collect();
        let is_known_department =
            |id: &Option<Id>| id.as_ref().is_none_or(|one| departments.contains(one));
        if !self.departments.iter().all(|d| {
            label_ok(&d.label)
                && is_known_department(&d.parent)
                && d.head.as_ref().is_none_or(|h| people.contains(h))
        }) {
            return Err("部署の名前か、親か長が見つからない".into());
        }
        if !self
            .people
            .iter()
            .all(|p| label_ok(&p.label) && is_known_department(&p.department))
        {
            return Err("人の名前か部署が見つからない".into());
        }
        if !self.teams.iter().all(|t| {
            label_ok(&t.label) && is_known_department(&t.department) && is_node_origin(&t.node)
        }) {
            return Err("チームの名前か、部署か台の URL の形が違う".into());
        }
        Ok(())
    }

    /// 次の版の下書き: 版を 1 つ進め、前の版の指紋でつなぐ。中身は呼ぶ側が書き換える
    pub fn successor_of(current: &SignedOrg, at: String) -> Self {
        Self {
            version: current.body.version + 1,
            prev: Some(current.digest()),
            at,
            ..current.body.clone()
        }
    }
}

/// 1 版目: 根の鍵で署名してあり、形が合う
pub fn verify_genesis(doc: &SignedOrg) -> Result<(), String> {
    let body = &doc.body;
    if body.version != 1
        || doc.signer != body.root.account
        || !doc.verify(Kind::Org, &body.root.public_key)
    {
        return Err("1 版目の署名が合わない".into());
    }
    body.validate()
}

/// 今の版の次の版か: 版が 1 大きく、前の版の指紋でつながり、今の根の鍵で署名してある
pub fn verify_successor(current: &SignedOrg, next: &SignedOrg) -> Result<(), String> {
    let (now, body) = (&current.body, &next.body);
    if body.org != now.org
        || body.version != now.version + 1
        || body.prev.as_deref() != Some(current.digest().as_str())
    {
        return Err(format!("{} の {} 版の次の版ではない", now.org, now.version));
    }
    if next.signer != now.root.account || !next.verify(Kind::Org, &now.root.public_key) {
        return Err("今の根の鍵の署名ではない".into());
    }
    body.validate()
}
