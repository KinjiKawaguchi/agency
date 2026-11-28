//! The teams a node hosts: who owns each, what runs it, and where its delivery numbers come from.
use super::{DeliveryConfig, DeliverySite};
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Account {
    pub id: Id,
    pub label: String,
}

/// A team this node hosts, and the command that runs its agency-runtime/1 runtime. Made with
/// `HostedTeam::new`, so a new setting gets its default in one place and not in every place that builds one
/// (other crates cannot build it by hand; a test keeps this crate from doing it either).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct HostedTeam {
    pub org: Id,
    pub team: Id,
    /// The org person who owns the team. Requests without an addressee go to them.
    pub owner: Id,
    /// Runtime names of other addressees, mapped to org person ids.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub people: BTreeMap<String, Id>,
    pub runtime: RuntimeCommand,
    /// Where the team's delivery numbers come from. Without it, the team has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<DeliveryConfig>,
}

impl HostedTeam {
    /// A team of `org` owned by `owner`, run by `runtime`, with no other addressees.
    pub fn new(org: Id, team: Id, owner: Id, runtime: RuntimeCommand) -> Self {
        Self {
            org,
            team,
            owner,
            people: BTreeMap::new(),
            runtime,
            delivery: None,
        }
    }

    /// The same team with these addressees: runtime names mapped to org person ids.
    pub fn with_people(self, people: BTreeMap<String, Id>) -> Self {
        Self { people, ..self }
    }

    /// The same team, whose delivery numbers are worked out from these GitHub repositories (`owner/name`)
    /// on github.com.
    pub fn with_delivery(self, repos: Vec<String>) -> Self {
        Self {
            delivery: Some(DeliveryConfig {
                repos,
                host: None,
                team_field: None,
                team_value: None,
            }),
            ..self
        }
    }

    /// The same team, counting only the pull requests whose issue has `value` in the board's `field`
    /// (a team that shares its repositories with others). It changes nothing for a team without delivery.
    pub fn with_team_value(self, field: impl Into<String>, value: impl Into<String>) -> Self {
        let delivery = self.delivery.map(|delivery| DeliveryConfig {
            team_field: Some(field.into()),
            team_value: Some(value.into()),
            ..delivery
        });
        Self { delivery, ..self }
    }

    /// The same team, whose delivery numbers are worked out from these repositories (`owner/name`) on
    /// the GitHub host `host` (a host name, for a GitHub Enterprise Server).
    pub fn with_delivery_on(self, host: impl Into<String>, repos: Vec<String>) -> Self {
        Self {
            delivery: Some(DeliveryConfig {
                repos,
                host: Some(host.into()),
                team_field: None,
                team_value: None,
            }),
            ..self
        }
    }

    /// What a read of the team's delivery numbers depends on, when the configuration names at least one
    /// repository: no repository, no source.
    pub fn delivery_source(&self) -> Option<DeliverySite> {
        let delivery = self.delivery.as_ref()?;
        if delivery.repos.is_empty() {
            return None;
        }
        let site = DeliverySite::new(delivery.repos.clone());
        Some(match &delivery.host {
            Some(host) => site.with_host(host),
            None => site,
        })
    }

    /// The same team run by another command.
    pub fn with_runtime(self, runtime: RuntimeCommand) -> Self {
        Self { runtime, ..self }
    }

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
    /// A command with no environment.
    pub fn of(command: Vec<String>) -> Self {
        Self {
            command,
            env: BTreeMap::new(),
        }
    }

    /// The same command with this environment.
    pub fn with_env(self, env: BTreeMap<String, String>) -> Self {
        Self { env, ..self }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn requests_go_to_mapped_people_and_otherwise_to_the_owner() {
        let runtime = RuntimeCommand::of(vec!["true".into()]);
        let team = HostedTeam::new(id("acme"), id("app"), id("alice"), runtime)
            .with_people(BTreeMap::from([("reviewer".to_owned(), id("bob"))]));
        assert_eq!(team.addressee(None), &id("alice"));
        assert_eq!(team.addressee(Some("reviewer")), &id("bob"));
        assert_eq!(team.addressee(Some("someone-else")), &id("alice"));
    }
}
