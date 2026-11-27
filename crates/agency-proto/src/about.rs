//! What a node says about itself, so an app or another node can check what it speaks before relying on
//! something new (docs/compatibility.md, "Discovery"). Readers ignore what they do not know: an answer is
//! read without `deny_unknown_fields`, and a capability is a plain string, so a newer node may list a name
//! this one has never heard of.
use crate::org::OrgDefinition;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;

/// The `/fed/vN` versions this release serves.
pub const FEDERATION_VERSIONS: &[u32] = &[1];
/// The `agency/vN/<kind>` signed-text versions this release accepts and writes.
pub const SIGNED_TEXT_VERSIONS: &[u32] = &[1];
/// The `agency-runtime/N` versions this release speaks to a team runtime.
pub const RUNTIME_VERSIONS: &[u32] = &[1];
/// The Client API version this release serves, and the oldest one it still serves.
pub const CLIENT_API_CURRENT: u32 = 1;
pub const CLIENT_API_OLDEST: u32 = 1;

/// The Client API versions a node serves: its own, and the oldest it still answers as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientApiRange {
    pub current: u32,
    pub oldest: u32,
}

/// The contract versions a node speaks, each a list so that two can be served side by side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Contracts {
    pub federation: Vec<u32>,
    pub signed_text: Vec<u32>,
    pub runtime: Vec<u32>,
    pub client_api: ClientApiRange,
}

/// `GET /api/about` and `GET /fed/v1/orgs/<org>/about`. The federation answer never has `oldestApp`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct About {
    /// The node's release, `MAJOR.MINOR.PATCH`.
    pub node: String,
    pub contracts: Contracts,
    /// The names of the optional features and signed fields this node understands. A reader ignores a name
    /// it does not know.
    pub capabilities: Vec<String>,
    /// For each platform (`ios`, `macos`), the oldest app version the node serves. Absent when the node
    /// sets no floor, which is always the case until a breaking Client API change needs one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_app: Option<BTreeMap<String, String>>,
}

/// The most a node keeps of another node's `About`: this many versions of a contract, capabilities, and
/// platforms, and this many characters in any text of it.
pub const MAX_ENTRIES: usize = 64;
pub const MAX_TEXT: usize = 64;

impl About {
    /// Whether this is an answer to keep: every text is short and without control characters, and no list
    /// is long. A node keeps what other nodes tell it, so it keeps no more than it can hold.
    pub fn check(&self) -> Result<(), String> {
        let is_text = |raw: &str| {
            !raw.is_empty() && raw.chars().count() <= MAX_TEXT && !raw.chars().any(char::is_control)
        };
        let contracts = &self.contracts;
        let versions = [
            &contracts.federation,
            &contracts.signed_text,
            &contracts.runtime,
        ];
        let floor = self.oldest_app.iter().flatten();
        if !is_text(&self.node)
            || versions.iter().any(|list| list.len() > MAX_ENTRIES)
            || self.capabilities.len() > MAX_ENTRIES
            || !self.capabilities.iter().all(|name| is_text(name))
            || self
                .oldest_app
                .as_ref()
                .is_some_and(|apps| apps.len() > MAX_ENTRIES)
            || !floor
                .into_iter()
                .all(|(platform, version)| is_text(platform) && is_text(version))
        {
            return Err("sent an answer beyond the limits".into());
        }
        Ok(())
    }

    /// What a node of this release says: its version, every contract this release speaks, and every
    /// capability it has. No floor for apps.
    pub fn of_release(node: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            contracts: Contracts {
                federation: FEDERATION_VERSIONS.to_vec(),
                signed_text: SIGNED_TEXT_VERSIONS.to_vec(),
                runtime: RUNTIME_VERSIONS.to_vec(),
                client_api: ClientApiRange {
                    current: CLIENT_API_CURRENT,
                    oldest: CLIENT_API_OLDEST,
                },
            },
            capabilities: Capability::ALL
                .iter()
                .map(|one| one.name().to_owned())
                .collect(),
            oldest_app: None,
        }
    }

    /// Whether the node lists `capability`.
    pub fn lists(&self, capability: Capability) -> bool {
        self.capabilities
            .iter()
            .any(|name| name == capability.name())
    }
}

/// An optional feature, or an optional field of a signed document that older nodes refuse. Names start at
/// what came after the first release: every node has what the first release had. The meaning of each is in
/// docs/compatibility.md ("Discovery"), and a test keeps that table and this list the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// `teams[].plan` in the org definition.
    OrgPlan,
    /// `teams[].plan.aims` in the org definition.
    OrgAims,
    /// `kpis` in the org definition.
    OrgKpis,
    /// `root.recovery` and `root.pending` in the org definition: the recovery key and a started recovery.
    OrgRecovery,
    /// `seats` in the org definition: places in the org that no one has taken yet, and the seats a member
    /// took (`people[].formerly`, `people[].askAbout`).
    OrgSeats,
    /// `externals` in the org definition: people the org works with who never take a seat.
    OrgExternals,
    /// `people[].hostedBy` in the org definition: members with no node of their own, hosted by the root's
    /// node, whose `publicKey` is left out.
    HostedMembers,
    /// A browser's Web Push subscriptions and the notices sent to them.
    WebPush,
    /// A team's activity feed.
    Activity,
    /// The owner's numbers and the readings of them.
    Outcomes,
    /// `device` in a push registration (an APNs token or a Web Push subscription), and the registrations
    /// dropped with the device they name.
    PushDevice,
    /// Where a team's tokens went, by work type: `GET /api/orgs/<org>/effort` and the federation read of it.
    Effort,
    /// A signed read that names the node it is made for (`x-agency-audience`, `agency/v2/read`).
    ReadAudience,
    /// The org's members, their devices, its open seats and its held changes, for the owner: `GET /api/orgs/<org>/members`.
    Members,
}

impl Capability {
    pub const ALL: [Capability; 14] = [
        Capability::OrgPlan,
        Capability::OrgAims,
        Capability::OrgKpis,
        Capability::OrgRecovery,
        Capability::OrgSeats,
        Capability::OrgExternals,
        Capability::HostedMembers,
        Capability::WebPush,
        Capability::Activity,
        Capability::Outcomes,
        Capability::PushDevice,
        Capability::Effort,
        Capability::ReadAudience,
        Capability::Members,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Capability::OrgPlan => "orgPlan",
            Capability::OrgAims => "orgAims",
            Capability::OrgKpis => "orgKpis",
            Capability::OrgRecovery => "orgRecovery",
            Capability::OrgSeats => "orgSeats",
            Capability::OrgExternals => "orgExternals",
            Capability::HostedMembers => "hostedMembers",
            Capability::WebPush => "webPush",
            Capability::Activity => "activity",
            Capability::Outcomes => "outcomes",
            Capability::PushDevice => "pushDevice",
            Capability::Effort => "effort",
            Capability::ReadAudience => "readAudience",
            Capability::Members => "members",
        }
    }

    /// The capability a name stands for, or `None` for one this release does not know.
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|one| one.name() == raw)
    }

    /// The capabilities a reader needs to read `definition`: one for each optional field it carries. A
    /// node refuses a version with a field it does not know, so the root checks these before signing.
    pub fn used_by(definition: &OrgDefinition) -> BTreeSet<Capability> {
        let plans = definition
            .teams
            .iter()
            .filter_map(|team| team.plan.as_ref());
        let mut used = BTreeSet::new();
        for plan in plans {
            used.insert(Capability::OrgPlan);
            if !plan.aims.is_empty() {
                used.insert(Capability::OrgAims);
            }
        }
        if !definition.kpis.is_empty() {
            used.insert(Capability::OrgKpis);
        }
        let root = &definition.root;
        if root.recovery.is_some() || root.pending.is_some() {
            used.insert(Capability::OrgRecovery);
        }
        if !definition.seats.is_empty() {
            used.insert(Capability::OrgSeats);
        }
        if !definition.externals.is_empty() {
            used.insert(Capability::OrgExternals);
        }
        if definition.people.iter().any(|one| one.hosted_by.is_some()) {
            used.insert(Capability::HostedMembers);
        }
        let took_a_seat =
            |one: &crate::org::Person| !one.formerly.is_empty() || !one.ask_about.is_empty();
        if definition.people.iter().any(took_a_seat) {
            used.insert(Capability::OrgSeats);
        }
        used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const SAMPLE: &str = include_str!("../../../testdata/about.sample.json");
    const COMPATIBILITY: &str = include_str!("../../../docs/compatibility.md");
    const VECTORS: &str = include_str!("../../../testdata/vectors.json");

    /// The paths of the org definition's JSON that the first release (v0.1.0) already had, as far as the
    /// org vectors show them (`departments[].parent` and the `people[].devices` paths are there too, with no
    /// vector that carries them yet). A path a vector shows that is in neither list fails the test below.
    const FIRST_RELEASE: &[&str] = &[
        "at",
        "departments",
        "departments[].head",
        "departments[].id",
        "departments[].label",
        "departments[].parent",
        "label",
        "org",
        "people",
        "people[].account",
        "people[].department",
        "people[].devices",
        "people[].devices[].addedAt",
        "people[].devices[].id",
        "people[].devices[].label",
        "people[].devices[].publicKey",
        "people[].id",
        "people[].label",
        "people[].publicKey",
        "people[].role",
        "prev",
        "revoked",
        "root",
        "root.account",
        "root.publicKey",
        "teams",
        "teams[].department",
        "teams[].id",
        "teams[].label",
        "teams[].node",
        "version",
    ];

    /// The paths added after the first release, each with the capability a reader needs for it.
    const AFTER_FIRST_RELEASE: &[(&str, Capability)] = &[
        ("root.recovery", Capability::OrgRecovery),
        ("root.pending", Capability::OrgRecovery),
        ("root.pending.publicKey", Capability::OrgRecovery),
        ("root.pending.recovery", Capability::OrgRecovery),
        ("externals", Capability::OrgExternals),
        ("people[].hostedBy", Capability::HostedMembers),
        ("externals[].askAbout", Capability::OrgExternals),
        ("externals[].id", Capability::OrgExternals),
        ("externals[].label", Capability::OrgExternals),
        ("externals[].relays", Capability::OrgExternals),
        ("people[].askAbout", Capability::OrgSeats),
        ("people[].formerly", Capability::OrgSeats),
        ("seats", Capability::OrgSeats),
        ("seats[].askAbout", Capability::OrgSeats),
        ("seats[].department", Capability::OrgSeats),
        ("seats[].id", Capability::OrgSeats),
        ("seats[].label", Capability::OrgSeats),
        ("seats[].relays", Capability::OrgSeats),
        ("kpis", Capability::OrgKpis),
        ("kpis[].currency", Capability::OrgKpis),
        ("kpis[].goodWhen", Capability::OrgKpis),
        ("kpis[].id", Capability::OrgKpis),
        ("kpis[].kind", Capability::OrgKpis),
        ("kpis[].label", Capability::OrgKpis),
        ("kpis[].unit", Capability::OrgKpis),
        ("teams[].plan", Capability::OrgPlan),
        ("teams[].plan.aims", Capability::OrgAims),
        ("teams[].plan.aims[].epic", Capability::OrgAims),
        ("teams[].plan.aims[].kpi", Capability::OrgAims),
        ("teams[].plan.allocation", Capability::OrgPlan),
        ("teams[].plan.allocation.debt", Capability::OrgPlan),
        ("teams[].plan.allocation.improvement", Capability::OrgPlan),
        ("teams[].plan.allocation.incident", Capability::OrgPlan),
        ("teams[].plan.allocation.maintenance", Capability::OrgPlan),
        ("teams[].plan.allocation.newFeature", Capability::OrgPlan),
        ("teams[].plan.weeklyBudgetUsd", Capability::OrgPlan),
    ];

    /// Every org definition in the vectors, wherever it is, named by its path (`rootRotation.namingTheNewRoot`),
    /// so a vector added under any name is checked.
    fn org_vectors() -> Vec<(String, Value)> {
        let vectors: Value = serde_json::from_str(VECTORS).unwrap();
        let mut found = vec![];
        org_definitions_in(&vectors, "", &mut found);
        found
    }

    fn org_definitions_in(value: &Value, at: &str, found: &mut Vec<(String, Value)>) {
        let Value::Object(fields) = value else { return };
        let canonical = fields.get("canonical").and_then(Value::as_str);
        if let Some(body) = canonical.and_then(|text| serde_json::from_str::<Value>(text).ok()) {
            if serde_json::from_value::<OrgDefinition>(body.clone()).is_ok() {
                found.push((at.to_owned(), body));
            }
        }
        for (name, inner) in fields {
            let path = if at.is_empty() {
                name.clone()
            } else {
                format!("{at}.{name}")
            };
            org_definitions_in(inner, &path, found);
        }
    }

    fn paths(value: &Value, prefix: &str, found: &mut BTreeSet<String>) {
        match value {
            Value::Object(fields) => {
                for (name, inner) in fields {
                    let path = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{prefix}.{name}")
                    };
                    found.insert(path.clone());
                    paths(inner, &path, found);
                }
            }
            Value::Array(items) => {
                for item in items {
                    paths(item, &format!("{prefix}[]"), found);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn the_sample_is_what_a_release_says_and_has_no_floor_for_apps() {
        let sample: About = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(sample, About::of_release("0.3.0"));
        let again = serde_json::to_value(&sample).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        assert!(again.get("oldestApp").is_none(), "left out, never null");
        for one in Capability::ALL {
            assert!(sample.lists(one), "{one:?}");
        }
    }

    #[test]
    fn a_reader_ignores_what_it_does_not_know() {
        let mut newer: Value = serde_json::from_str(SAMPLE).unwrap();
        newer["capabilities"] = json!(["orgPlan", "someFutureThing"]);
        newer["contracts"]["federation"] = json!([1, 2]);
        newer["somethingNew"] = json!({ "a": 1 });
        newer["oldestApp"] = json!({ "ios": "0.4.0" });
        let read: About = serde_json::from_value(newer).unwrap();
        assert!(read.lists(Capability::OrgPlan) && !read.lists(Capability::OrgKpis));
        assert_eq!(Capability::parse("someFutureThing"), None);
        assert_eq!(read.contracts.federation, [1, 2]);
        assert_eq!(read.oldest_app.unwrap()["ios"], "0.4.0");
    }

    #[test]
    fn an_answer_is_kept_only_within_the_limits() {
        let sample = || About::of_release("0.3.0");
        assert_eq!(sample().check(), Ok(()));
        let refused = |edit: &dyn Fn(&mut About)| {
            let mut about = sample();
            edit(&mut about);
            about.check().is_err()
        };
        assert!(refused(&|a| a.node = String::new()));
        assert!(refused(&|a| a.node = "x".repeat(MAX_TEXT + 1)));
        assert!(refused(&|a| a.node = "0.3\n0".into()));
        assert!(refused(
            &|a| a.capabilities = vec!["a".into(); MAX_ENTRIES + 1]
        ));
        assert!(refused(&|a| a.capabilities.push(String::new())));
        assert!(refused(
            &|a| a.contracts.federation = vec![1; MAX_ENTRIES + 1]
        ));
        assert!(refused(
            &|a| a.oldest_app = Some(BTreeMap::from([("ios".into(), "\u{7}".into())]))
        ));
        let at_limit = || {
            let mut about = sample();
            about.node = "x".repeat(MAX_TEXT);
            about.capabilities = vec!["a".into(); MAX_ENTRIES];
            about
        };
        assert_eq!(at_limit().check(), Ok(()), "the limits are inclusive");
    }

    #[test]
    fn names_are_unique_and_each_parses_back() {
        let names: BTreeSet<&str> = Capability::ALL.iter().map(|one| one.name()).collect();
        assert_eq!(names.len(), Capability::ALL.len());
        for one in Capability::ALL {
            assert_eq!(Capability::parse(one.name()), Some(one));
        }
        assert_eq!(Capability::parse("OrgPlan"), None, "names are exact");
    }

    #[test]
    fn the_policys_table_lists_exactly_the_names_the_code_has() {
        let table = COMPATIBILITY.split("### Capabilities").nth(1).unwrap();
        let section = table.split("\n## ").next().unwrap();
        let listed: BTreeSet<&str> = section
            .lines()
            .filter_map(|line| line.strip_prefix("| `"))
            .filter_map(|rest| rest.split('`').next())
            .collect();
        let coded: BTreeSet<&str> = Capability::ALL.iter().map(|one| one.name()).collect();
        assert_eq!(listed, coded);
    }

    #[test]
    fn a_definition_needs_a_capability_for_each_optional_field_it_carries() {
        let used = |name: &str| {
            let (_, body) = org_vectors()
                .into_iter()
                .find(|(one, _)| one == name)
                .unwrap();
            let definition: OrgDefinition = serde_json::from_value(body).unwrap();
            Capability::used_by(&definition)
        };
        assert_eq!(used("org"), BTreeSet::new());
        assert_eq!(
            used("rootRecovery.start"),
            BTreeSet::from([Capability::OrgRecovery])
        );
        assert_eq!(used("rootRotation.namingTheNewRoot"), BTreeSet::new());
        assert_eq!(used("orgWithPlan"), BTreeSet::from([Capability::OrgPlan]));
        assert_eq!(
            used("orgWithSeats"),
            BTreeSet::from([Capability::OrgSeats, Capability::OrgExternals])
        );
        assert_eq!(
            used("orgWithClaimedSeat"),
            BTreeSet::from([Capability::OrgSeats]),
            "a member who took a seat needs the capability although `seats` is empty"
        );
        assert_eq!(
            used("orgWithOutcomes"),
            BTreeSet::from([
                Capability::OrgPlan,
                Capability::OrgAims,
                Capability::OrgKpis
            ])
        );
    }

    #[test]
    fn no_optional_field_of_the_definition_goes_without_a_capability() {
        let mut found = BTreeSet::new();
        for (_, body) in org_vectors() {
            paths(&body, "", &mut found);
        }
        let known: BTreeSet<&str> = FIRST_RELEASE
            .iter()
            .copied()
            .chain(AFTER_FIRST_RELEASE.iter().map(|(path, _)| *path))
            .collect();
        let unnamed: Vec<&String> = found
            .iter()
            .filter(|path| !known.contains(path.as_str()))
            .collect();
        assert!(
            unnamed.is_empty(),
            "a field of the org definition has no capability: {unnamed:?}. A field added after v0.1.0 \
             needs a `Capability`, a row in docs/compatibility.md, and an entry in AFTER_FIRST_RELEASE; \
             one the first release had goes in FIRST_RELEASE."
        );
        for (path, _) in AFTER_FIRST_RELEASE {
            assert!(
                found.contains(*path),
                "no vector has {path}: a stale entry, or a vector to add"
            );
        }
    }
}
