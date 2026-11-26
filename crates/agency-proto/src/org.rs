//! Org definitions. Only the org's root key signs them; versions grow by one and link to the previous version's fingerprint (one writer).
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::signed::{Kind, Signed};
use crate::timestamp::Timestamp;
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
    /// The account key, held by the person's home node. It signs reads, join requests, and the first
    /// device, never decisions.
    pub public_key: PublicKey,
    pub department: Option<Id>,
    pub role: Role,
    /// Keys held by the person's clients. Only these sign decisions. Left out of JSON when empty, so
    /// versions signed before devices existed keep their signatures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Device>>", optional)]
    pub devices: Vec<Device>,
}

/// A key held by one of a person's clients (a browser, a phone, the CLI). Its private key never leaves
/// that client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Device {
    /// Unique among the person's devices; envelopes name it as their `signer`.
    pub id: Id,
    pub label: String,
    pub public_key: PublicKey,
    pub added_at: Timestamp,
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

fn unique_keys<'a>(keys: impl Iterator<Item = &'a PublicKey>) -> bool {
    let mut seen = HashSet::new();
    keys.into_iter().all(|key| seen.insert(key))
}

impl OrgDefinition {
    /// A member: listed in the definition, with a key that is not revoked.
    pub fn member(&self, account: &Id) -> Option<&Person> {
        self.people
            .iter()
            .find(|one| &one.account == account && !self.revoked.contains(&one.public_key))
    }

    /// A device of a member, which may sign for them. A well-formed definition never lists a revoked
    /// device key (see `validate`), so a listed device is a usable one.
    pub fn device(&self, account: &Id, device: &Id) -> Option<(&Person, &Device)> {
        let person = self.member(account)?;
        let found = person.devices.iter().find(|one| &one.id == device)?;
        Some((person, found))
    }

    /// Account and root keys: these must never be device keys, because nodes hold them.
    fn node_keys(&self) -> HashSet<&PublicKey> {
        let accounts = self.people.iter().map(|p| &p.public_key);
        accounts.chain([&self.root.public_key]).collect()
    }

    /// Device ids are unique per person; device keys are unique in the org, are not account or root
    /// keys, and are not revoked.
    fn validate_devices(&self) -> Result<(), String> {
        let node_keys = self.node_keys();
        let devices: Vec<&Device> = self.people.iter().flat_map(|p| &p.devices).collect();
        let is_usable = |device: &&Device| {
            label_ok(&device.label)
                && !node_keys.contains(&device.public_key)
                && !self.revoked.contains(&device.public_key)
        };
        if !devices.iter().all(is_usable)
            || !unique_keys(devices.iter().map(|d| &d.public_key))
            || !self
                .people
                .iter()
                .all(|p| unique(p.devices.iter().map(|d| &d.id)))
        {
            return Err("a device has a malformed label, a repeated id or key, or a key that is an account, root, or revoked key".into());
        }
        Ok(())
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
        let are_node_keys_ed25519 = self.root.public_key.is_ed25519()
            && self.people.iter().all(|p| p.public_key.is_ed25519());
        if !are_node_keys_ed25519 {
            return Err("account and root keys must be Ed25519".into());
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
        if self.has_department_cycle() {
            return Err("a department is under itself".into());
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
        self.validate_devices()
    }

    /// Whether following parents from some department leads back to it. Departments form a tree.
    fn has_department_cycle(&self) -> bool {
        let parent_of = |id: &Id| {
            self.departments
                .iter()
                .find(|one| &one.id == id)
                .and_then(|one| one.parent.as_ref())
        };
        self.departments.iter().any(|start| {
            let mut at = start.parent.as_ref();
            for _ in 0..self.departments.len() {
                match at {
                    Some(id) if id == &start.id => return true,
                    Some(id) => at = parent_of(id),
                    None => return false,
                }
            }
            at.is_some()
        })
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
