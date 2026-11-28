//! The node's configuration (`config.json` in its data directory): its account, its URL, the teams it hosts, and the orgs it trusts.
//! The settings of each kind are in their own part and are reached from here.
mod access;
mod delivery;
mod funding;
mod push;
mod teams;

pub use access::AccessConfig;
pub use delivery::{is_host_name, DeliveryConfig, DeliverySite};
pub use funding::{FundingConfig, Tool, UsageCollector};
pub use push::{PushConfig, WebPushConfig};
pub use teams::{Account, HostedTeam, RuntimeCommand};

use crate::federation_config::FederationConfig;
use crate::logging_config::LoggingConfig;
use crate::metrics_config::MetricsConfig;
use crate::oidc_config::OidcConfig;
use crate::outcomes_config::OutcomesConfig;
use agency_proto::Id;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
#[non_exhaustive]
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
    /// An OpenID Connect provider in front of the Client API, in place of `access`. A node has one or neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oidc: Option<OidcConfig>,
    /// Apple Push Notification service, for the owner's iPhone. Without it, nothing is sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<PushConfig>,
    /// Web Push, for the owner's browsers. Without it, the node has no subscriptions and sends nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_push: Option<WebPushConfig>,
    /// Leave every title and label out of every push notification, whichever service carries it (APNs or a
    /// browser's push service), for a node whose data must stay where it is. `push.hideTitles` has the
    /// same meaning and is still read.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_titles: bool,
    /// The owner's other home nodes, for their other accounts, which clients offer to switch to. The
    /// node says only where they are; it never reads or copies anything from them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub home_nodes: Vec<HomeNodeConfig>,
    /// The node's log: its level, and when its files are rotated and deleted.
    #[serde(default, skip_serializing_if = "LoggingConfig::is_default")]
    pub logging: LoggingConfig,
    /// The node's measurements: the objectives its operator sets for how fast a group of routes should answer.
    #[serde(default, skip_serializing_if = "MetricsConfig::is_default")]
    pub metrics: MetricsConfig,
    /// The KPIs whose daily number the node works out itself, from its delivery numbers.
    #[serde(default, skip_serializing_if = "OutcomesConfig::is_default")]
    pub outcomes: OutcomesConfig,
    /// Where the org root keys this node holds are kept.
    #[serde(default, skip_serializing_if = "RootKeyStore::is_default")]
    pub root_keys: RootKeyStore,
    /// How the node takes the signed reads of other nodes.
    #[serde(default, skip_serializing_if = "FederationConfig::is_default")]
    pub federation: FederationConfig,
    /// Whether the Client API and the web client admit direct local requests (loopback, a local `Host`).
    /// `false` refuses every one of them with 403, for a machine that other accounts log in to: each of them
    /// reaches the loopback address as well. The federation routes answer either way.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub local_clients: bool,
}

/// Where a node keeps the org root keys it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RootKeyStore {
    /// Files outside the data directory, readable by the owner's OS user only.
    #[default]
    Files,
    /// The macOS login keychain, with Touch ID or the login password before each root signature.
    Keychain,
}

impl RootKeyStore {
    fn is_default(&self) -> bool {
        *self == Self::Files
    }
}

/// One of the owner's other home nodes, as configured: a label and its node origin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HomeNodeConfig {
    pub label: String,
    pub url: String,
}

impl HomeNodeConfig {
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

impl NodeConfig {
    /// A node's configuration with no hosted teams, trusted orgs, funding sources, Access, push, Web Push,
    /// or other home nodes.
    pub fn new(
        url: impl Into<String>,
        listen: impl Into<String>,
        account: Account,
        scrum: impl Into<PathBuf>,
    ) -> Self {
        Self {
            url: url.into(),
            listen: listen.into(),
            account,
            scrum: scrum.into(),
            teams: vec![],
            trusted: vec![],
            funding: vec![],
            access: None,
            oidc: None,
            push: None,
            web_push: None,
            hide_titles: false,
            home_nodes: vec![],
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            outcomes: OutcomesConfig::default(),
            root_keys: RootKeyStore::Files,
            federation: FederationConfig::default(),
            local_clients: true,
        }
    }

    /// The same configuration with `oidc` in front of the Client API.
    pub fn with_oidc(self, oidc: OidcConfig) -> Self {
        Self {
            oidc: Some(oidc),
            ..self
        }
    }

    /// Whether a sign-in provider (`access` or `oidc`) is set, well formed or not: with one, a direct local
    /// request may read but never write, and a malformed one must not make loopback the owner again.
    pub fn has_sign_in(&self) -> bool {
        self.access.is_some() || self.oidc.is_some()
    }

    /// The host name requests from outside name, from whichever sign-in provider is set.
    pub fn public_host(&self) -> Option<&str> {
        self.access
            .as_ref()
            .map(|access| access.public_host.as_str())
            .or_else(|| self.oidc.as_ref().map(|oidc| oidc.public_host.as_str()))
    }

    /// Whether the Client API admits direct local requests (`localClients`).
    pub fn with_local_clients(self, local_clients: bool) -> Self {
        Self {
            local_clients,
            ..self
        }
    }

    /// How the node takes the signed reads of other nodes (`federation`).
    pub fn with_federation(self, federation: FederationConfig) -> Self {
        Self { federation, ..self }
    }

    /// The owner's other home nodes (`homeNodes`).
    pub fn with_home_nodes(self, home_nodes: Vec<HomeNodeConfig>) -> Self {
        Self { home_nodes, ..self }
    }

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

    /// Moves the trust in `org` from one root key to the next, once a version that names the new root
    /// (signed by the old one) was accepted. Only the trusted fingerprint `from` is replaced.
    pub fn retrust(&mut self, org: &Id, from: &str, to: String) {
        let trusted = self
            .trusted
            .iter_mut()
            .find(|one| &one.org == org && one.root_fingerprint == from);
        if let Some(one) = trusted {
            one.root_fingerprint = to;
        }
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
        let account = Account {
            id: id("alice"),
            label: "Alice".into(),
        };
        let mut config =
            NodeConfig::new("http://127.0.0.1:4400", "127.0.0.1:4400", account, "scrum");
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
}
