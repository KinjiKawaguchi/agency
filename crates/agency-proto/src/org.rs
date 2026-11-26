//! Org definitions. Only the org's root key signs them; versions grow by one and link to the previous version's fingerprint (one writer).
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::outcome::{Aim, Kpi};
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
    /// The recovery key: a second key, kept only as written words, that may move the root after a wait
    /// (`RECOVERY_WAIT_SECONDS`) and do nothing else. Left out when the org has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recovery: Option<PublicKey>,
    /// A recovery that was started and waits: what it will change. Only the recovery key sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pending: Option<PendingRoot>,
}

impl Root {
    /// A root with no recovery key.
    pub fn new(account: Id, public_key: PublicKey) -> Self {
        Self {
            account,
            public_key,
            recovery: None,
            pending: None,
        }
    }

    pub fn with_recovery(self, recovery: PublicKey) -> Self {
        Self {
            recovery: Some(recovery),
            ..self
        }
    }
}

/// What a started recovery changes in `root` once its wait is over: a new root key, a new recovery
/// key, or both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PendingRoot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub public_key: Option<PublicKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recovery: Option<PublicKey>,
}

/// How long, by its own clock, a node holds a started recovery before it takes the version that
/// finishes it: 72 hours, in which the current root cancels it by signing any version.
pub const RECOVERY_WAIT_SECONDS: i64 = 72 * 60 * 60;

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

/// A team and the node it runs on. Made with `TeamPlace::new`, so a new field gets its default in one place
/// and not in every place that builds one (other crates cannot build it by hand; a test keeps this crate
/// from doing it either).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
#[non_exhaustive]
pub struct TeamPlace {
    pub id: Id,
    pub label: String,
    pub department: Option<Id>,
    /// The node's origin (http or https, no path).
    pub node: String,
    /// What the org intends for the team. Absent when it states nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plan: Option<TeamPlan>,
}

impl TeamPlace {
    /// A team in no department, with no plan, that runs at the node `node`.
    pub fn new(id: Id, label: impl Into<String>, node: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            department: None,
            node: node.into(),
            plan: None,
        }
    }

    /// The same team in this department (or in none).
    pub fn with_department(self, department: Option<Id>) -> Self {
        Self { department, ..self }
    }

    /// The same team with this plan (or none).
    pub fn with_plan(self, plan: Option<TeamPlan>) -> Self {
        Self { plan, ..self }
    }
}

/// What an org intends for a team: the share of its effort for each kind of work, and how much it
/// means to spend a week.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TeamPlan {
    pub allocation: Allocation,
    /// The weekly spend the org intends, in whole US dollars, estimated at API prices like
    /// `TokenUsage.costUsd` (an integer, since the definition is signed as canonical JSON).
    pub weekly_budget_usd: u32,
    /// What the team's epics are meant to move: each epic paired with a KPI the org states. Left out when
    /// empty, so a plan signed before aims existed keeps its canonical JSON and signature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Aim>>", optional)]
    pub aims: Vec<Aim>,
}

impl TeamPlan {
    /// A plan with no aims.
    pub fn new(allocation: Allocation, weekly_budget_usd: u32) -> Self {
        Self {
            allocation,
            weekly_budget_usd,
            aims: vec![],
        }
    }

    /// The same plan with these aims.
    pub fn with_aims(self, aims: Vec<Aim>) -> Self {
        Self { aims, ..self }
    }
}

/// Percent of a team's effort for each kind of work. The five add up to 100.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Allocation {
    pub new_feature: u32,
    pub improvement: u32,
    pub maintenance: u32,
    pub debt: u32,
    pub incident: u32,
}

impl Allocation {
    pub fn total(&self) -> u64 {
        [
            self.new_feature,
            self.improvement,
            self.maintenance,
            self.debt,
            self.incident,
        ]
        .into_iter()
        .map(u64::from)
        .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
#[non_exhaustive]
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
    /// What the org measures: labels and ids only, never a number. Left out when empty, so a version signed
    /// before KPIs existed keeps its canonical JSON and signature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Kpi>>", optional)]
    pub kpis: Vec<Kpi>,
    pub at: String,
}

pub type SignedOrg = Signed<OrgDefinition>;

/// The answer of the versions route: an org definition's versions after a given one, oldest first.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    pub versions: Vec<SignedOrg>,
}

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

pub(crate) fn label_ok(label: &str) -> bool {
    let count = label.chars().count();
    (1..=80).contains(&count) && !label.chars().any(char::is_control)
}

pub(crate) fn unique<'a>(ids: impl Iterator<Item = &'a Id>) -> bool {
    let mut seen = HashSet::new();
    ids.into_iter().all(|id| seen.insert(id))
}

fn unique_keys<'a>(keys: impl Iterator<Item = &'a PublicKey>) -> bool {
    let mut seen = HashSet::new();
    keys.into_iter().all(|key| seen.insert(key))
}

impl OrgDefinition {
    /// Version 1 of an org: no departments, people, teams, KPIs, or revoked keys yet. Built through this and
    /// its `with_` methods, never by a struct literal, so a new field gets its default in one place.
    pub fn new(org: Id, label: String, root: Root, at: String) -> Self {
        Self {
            org,
            label,
            version: 1,
            prev: None,
            root,
            departments: vec![],
            people: vec![],
            teams: vec![],
            revoked: vec![],
            kpis: vec![],
            at,
        }
    }

    pub fn with_departments(self, departments: Vec<Department>) -> Self {
        Self {
            departments,
            ..self
        }
    }

    pub fn with_people(self, people: Vec<Person>) -> Self {
        Self { people, ..self }
    }

    pub fn with_teams(self, teams: Vec<TeamPlace>) -> Self {
        Self { teams, ..self }
    }

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

    /// Account, root, and recovery keys: these must never be device keys.
    fn node_keys(&self) -> HashSet<&PublicKey> {
        let accounts = self.people.iter().map(|p| &p.public_key);
        let pending = self.root.pending.iter();
        let roots = [Some(&self.root.public_key), self.root.recovery.as_ref()]
            .into_iter()
            .chain(pending.flat_map(|one| [one.public_key.as_ref(), one.recovery.as_ref()]))
            .flatten();
        accounts.chain(roots).collect()
    }

    /// A recovery key that is also the root key or an account key would be kept on a node, which defeats it; a
    /// new root key a started recovery names is no account key either.
    fn are_recovery_keys_apart(&self) -> bool {
        let root = &self.root;
        let pending = root.pending.as_ref();
        let new_root = pending.and_then(|one| one.public_key.as_ref());
        let accounts: HashSet<&PublicKey> = self.people.iter().map(|p| &p.public_key).collect();
        let held: HashSet<&PublicKey> = accounts
            .iter()
            .copied()
            .chain([Some(&root.public_key), new_root].into_iter().flatten())
            .collect();
        let recovery_keys = [
            root.recovery.as_ref(),
            pending.and_then(|one| one.recovery.as_ref()),
        ];
        let is_recovery_apart = recovery_keys
            .into_iter()
            .flatten()
            .all(|key| !held.contains(key));
        is_recovery_apart && new_root.is_none_or(|key| !accounts.contains(key))
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
        let are_node_keys_ed25519 = self.node_keys().into_iter().all(PublicKey::is_ed25519);
        if !self.are_recovery_keys_apart() {
            return Err(
                "a recovery key, or a key a started recovery names, is the root key or an account key"
                    .into(),
            );
        }
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
        let plans = self.teams.iter().filter_map(|t| t.plan.as_ref());
        if !plans.into_iter().all(|plan| plan.allocation.total() == 100) {
            return Err("a team's plan allocation does not add up to 100".into());
        }
        crate::outcome::validate(self)?;
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

/// Who may sign the next version, and what it then may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Succession {
    /// Signed by the root key: anything but the recovery key (which it may only add while there is
    /// none), and never a pending recovery, so it cancels one that waits.
    Root,
    /// Signed by the recovery key, starting a recovery: it only sets `root.pending`.
    RecoveryStart,
    /// Signed by the recovery key, finishing a recovery: it only applies `root.pending`. A node takes it
    /// only after holding the start for `RECOVERY_WAIT_SECONDS` by its own clock.
    RecoveryFinish,
}

/// Whether `next` is the next version of `now`: one version higher, linked to its fingerprint, signed
/// by the current root key or recovery key, and changing only what that key may change.
pub fn verify_successor(current: &SignedOrg, next: &SignedOrg) -> Result<Succession, String> {
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
    let succession = succession(current, next)?;
    body.validate()?;
    Ok(succession)
}

/// Whether `next` is the next version of `now` signed by the root key. A node that asked the root's
/// node for a change (a join, a device) takes only this from its answer; a recovery comes by gossip,
/// where the node counts its wait.
pub fn verify_root_successor(current: &SignedOrg, next: &SignedOrg) -> Result<(), String> {
    match verify_successor(current, next)? {
        Succession::Root => Ok(()),
        _ => Err("not signed by the current root key".into()),
    }
}

fn succession(current: &SignedOrg, next: &SignedOrg) -> Result<Succession, String> {
    let (now, body) = (&current.body.root, &next.body.root);
    let is_signed_by = |key: &PublicKey| next.signer == now.account && next.verify(Kind::Org, key);
    if is_signed_by(&now.public_key) {
        if body.pending.is_some() {
            return Err("the root key does not start a recovery".into());
        }
        if now.recovery.is_some() && body.recovery != now.recovery {
            return Err("only the recovery key replaces the recovery key".into());
        }
        return Ok(Succession::Root);
    }
    if !now.recovery.as_ref().is_some_and(is_signed_by) {
        return Err("not signed by the current root key".into());
    }
    let mut expected = OrgDefinition::successor_of(current, next.body.at.clone());
    let succession = match &now.pending {
        None => {
            let pending = body
                .pending
                .as_ref()
                .ok_or("a recovery start names no change")?;
            if pending.public_key.is_none() && pending.recovery.is_none() {
                return Err("a recovery start names no change".into());
            }
            expected.root.pending = Some(pending.clone());
            Succession::RecoveryStart
        }
        Some(pending) => {
            expected.root = Root {
                account: now.account.clone(),
                public_key: pending.public_key.clone().unwrap_or(now.public_key.clone()),
                recovery: pending.recovery.clone().or(now.recovery.clone()),
                pending: None,
            };
            Succession::RecoveryFinish
        }
    };
    if expected != next.body {
        return Err("the recovery key changes nothing but the root".into());
    }
    Ok(succession)
}
