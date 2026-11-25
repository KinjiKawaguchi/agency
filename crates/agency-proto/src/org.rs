//! Org definitions. Only the org's root key signs them; versions grow by one and link to the previous version's fingerprint (one writer).
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

/// Whether this is a node origin: http or https with a host and port only.
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
    /// A member: listed in the definition, with a key that is not revoked.
    pub fn member(&self, account: &Id) -> Option<&Person> {
        self.people
            .iter()
            .find(|one| &one.account == account && !self.revoked.contains(&one.public_key))
    }

    pub fn team(&self, id: &Id) -> Option<&TeamPlace> {
        self.teams.iter().find(|one| &one.id == id)
    }

    /// Checks the shape: label lengths, duplicate ids, and that departments, heads, people, and team places refer to each other correctly.
    pub fn validate(&self) -> Result<(), String> {
        if !label_ok(&self.label) || self.version == 0 || (self.version == 1) != self.prev.is_none()
        {
            return Err("malformed org label or version".into());
        }
        if !unique(self.departments.iter().map(|d| &d.id))
            || !unique(self.people.iter().map(|p| &p.id))
            || !unique(self.teams.iter().map(|t| &t.id))
            || !unique(self.people.iter().map(|p| &p.account))
        {
            return Err("a department, person, team, or account appears twice".into());
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
            return Err("malformed department label, or its parent or head is missing".into());
        }
        if !self
            .people
            .iter()
            .all(|p| label_ok(&p.label) && is_known_department(&p.department))
        {
            return Err("malformed person label, or their department is missing".into());
        }
        if !self.teams.iter().all(|t| {
            label_ok(&t.label) && is_known_department(&t.department) && is_node_origin(&t.node)
        }) {
            return Err("malformed team label, department, or node URL".into());
        }
        Ok(())
    }

    /// A draft of the next version: one version higher and linked to the previous version's fingerprint. The caller edits the contents.
    pub fn successor_of(current: &SignedOrg, at: String) -> Self {
        Self {
            version: current.body.version + 1,
            prev: Some(current.digest()),
            at,
            ..current.body.clone()
        }
    }
}

/// Version 1: signed by the root key and well formed.
pub fn verify_genesis(doc: &SignedOrg) -> Result<(), String> {
    let body = &doc.body;
    if body.version != 1
        || doc.signer != body.root.account
        || !doc.verify(Kind::Org, &body.root.public_key)
    {
        return Err("the signature of version 1 does not match".into());
    }
    body.validate()
}

/// Whether `next` is the next version of `now`: one version higher, linked to its fingerprint, and signed by the current root key.
pub fn verify_successor(current: &SignedOrg, next: &SignedOrg) -> Result<(), String> {
    let (now, body) = (&current.body, &next.body);
    if body.org != now.org
        || body.version != now.version + 1
        || body.prev.as_deref() != Some(current.digest().as_str())
    {
        return Err(format!(
            "not the next version after version {} of {}",
            now.version, now.org
        ));
    }
    if next.signer != now.root.account || !next.verify(Kind::Org, &now.root.public_key) {
        return Err("not signed by the current root key".into());
    }
    body.validate()
}
