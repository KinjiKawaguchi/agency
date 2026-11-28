//! The node's configuration (`config.json` in its data directory): its account, its URL, the teams it hosts, and the orgs it trusts.
use crate::federation_config::FederationConfig;
use crate::logging_config::LoggingConfig;
use crate::metrics_config::MetricsConfig;
use agency_proto::client::FundingKind;
use agency_proto::evaluation::{is_repository, MAX_REPOSITORIES};
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
            delivery: Some(DeliveryConfig { repos, host: None }),
            ..self
        }
    }

    /// The same team, whose delivery numbers are worked out from these repositories (`owner/name`) on
    /// the GitHub host `host` (a host name, for a GitHub Enterprise Server).
    pub fn with_delivery_on(self, host: impl Into<String>, repos: Vec<String>) -> Self {
        Self {
            delivery: Some(DeliveryConfig {
                repos,
                host: Some(host.into()),
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

impl WebPushConfig {
    /// Why the node cannot send with this setting: an entry of `extraHosts` that is not a host name (or `*.` and a
    /// domain) in lowercase, such as an address or a form of one (`0x7f.1`).
    pub fn check(&self) -> Result<(), String> {
        if let Some(subject) = &self.subject {
            let is_contact = subject
                .strip_prefix("mailto:")
                .or_else(|| subject.strip_prefix("https://"))
                .is_some_and(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace));
            if !is_contact {
                return Err(format!(
                    "webPush.subject: \"{subject}\" is not a mailto: or https:// address"
                ));
            }
        }
        match self
            .extra_hosts
            .iter()
            .find(|entry| !crate::web_push::is_host_pattern(entry))
        {
            Some(entry) => Err(format!(
                "webPush.extraHosts: \"{entry}\" is not a host name in lowercase, or `*.` and a domain"
            )),
            None => Ok(()),
        }
    }
}

/// Web Push: the node's own VAPID key lives in its data directory, never here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPushConfig {
    /// The contact a push service may reach the node's operator at: a `mailto:` or `https:` URL. Without
    /// it, the node's own origin (`url`) is used; a personal address is only ever what the owner writes here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Push services to allow besides the built-in ones (Google, Mozilla, Apple, Microsoft): a host name,
    /// or `*.` and a domain. Never an address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_hosts: Vec<String>,
}

/// The GitHub repositories (`owner/name`) a hosted team's delivery numbers are worked out from: its
/// merged pull requests, the issues they close, and the CI runs on their default branches. GitHub is
/// read with the machine's own `gh` login, read-only; the configuration holds no token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryConfig {
    pub repos: Vec<String>,
    /// The host the repositories are on: a host name, for a GitHub Enterprise Server. Without it,
    /// github.com. `gh` is told the host, and its login for that host is the one used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

/// The host that is read when a team names none.
const GITHUB_COM: &str = "github.com";

/// A GitHub host name: dot-separated labels of letters, digits and hyphens, none empty, none starting or
/// ending with a hyphen, at most 253 characters in all. No scheme, port or path. The host goes to `gh` as
/// the value of `--hostname`, so nothing else is let in.
pub fn is_host_name(host: &str) -> bool {
    let is_label = |label: &str| {
        (1..=63).contains(&label.len())
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    host.len() <= 253 && host.split('.').all(is_label)
}

impl DeliveryConfig {
    /// Why these settings cannot be used, in words that name the setting; `at` is where they sit
    /// (`teams[0].delivery`).
    pub fn check(&self, at: &str) -> Result<(), String> {
        // The repositories are what a report names, so they have its limits: at most `MAX_REPOSITORIES`, each
        // written `owner/name`, none twice (GitHub names ignore case).
        if self.repos.len() > MAX_REPOSITORIES {
            return Err(format!(
                "{at}.repos has more than {MAX_REPOSITORIES} entries"
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (index, repo) in self.repos.iter().enumerate() {
            if !is_repository(repo) {
                return Err(format!("{at}.repos[{index}] is not written owner/name"));
            }
            if !seen.insert(repo.to_ascii_lowercase()) {
                return Err(format!(
                    "{at}.repos[{index}] names a repository twice (names ignore case)"
                ));
            }
        }
        match &self.host {
            Some(host) if !is_host_name(host) => Err(format!(
                "{at}.host is not a host name (letters, digits, hyphens and dots; no scheme, port or path)"
            )),
            _ => Ok(()),
        }
    }
}

/// What a read of a team's delivery numbers depends on: the repositories (`owner/name`) and the GitHub host
/// they are on, and nothing else of the team's delivery setting. A read is kept by it, so two teams that
/// differ in anything else share one read. The host is `None` for github.com, and is in lower case,
/// since a host name is the same in any case.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DeliverySite {
    pub repos: Vec<String>,
    pub host: Option<String>,
}

impl DeliverySite {
    /// These repositories on github.com.
    pub fn new(repos: Vec<String>) -> Self {
        Self { repos, host: None }
    }

    /// The same repositories on the GitHub host `host`; github.com, in any case, is no host.
    pub fn with_host(self, host: impl AsRef<str>) -> Self {
        let host = host.as_ref().to_ascii_lowercase();
        let host = (host != GITHUB_COM).then_some(host);
        Self { host, ..self }
    }
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
    /// Leave every title and label out of a notification (the team's label, a request's or a decision's
    /// title, a device's label), for a node whose data must stay where it is. The notification then
    /// says only what kind it is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_titles: bool,
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
    /// New agent sessions leave this source for the next one when its 7-day window passes this many
    /// percent (1 to 100; default 80). The order of `funding` is the order of preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day_threshold: Option<u32>,
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

/// The program an agent session runs on a funding source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    Claude,
    Codex,
}

impl UsageCollector {
    pub fn tool(&self) -> Tool {
        match self {
            Self::Claude { .. } => Tool::Claude,
            Self::Codex { .. } => Tool::Codex,
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
        /// Whether the node runs `claude` once, within limits, to renew the login when its access token
        /// has expired and nobody has used the login (default true). It never renews a token itself.
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        renew: bool,
        /// The `claude` program that renews the login (default: `claude` on the node's `PATH`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<PathBuf>,
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

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
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
    /// Whether the app may sign in through its custom scheme (`via=scheme`), for a host that is not in the app
    /// build's associated domains. Off by default: the scheme is delivered to the app that starts the sign-in,
    /// whatever app that is, so an app of someone else's on the owner's device could start it and redeem the
    /// owner's token; the universal link of `via=link` reaches only the app the association file names.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_scheme_sign_in: bool,
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
            push: None,
            web_push: None,
            hide_titles: false,
            home_nodes: vec![],
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            root_keys: RootKeyStore::Files,
            federation: FederationConfig::default(),
            local_clients: true,
        }
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
    fn the_apps_scheme_sign_in_is_off_unless_the_access_setting_turns_it_on() {
        let base = r#""publicHost":"agency.example.com","teamDomain":"acme.cloudflareaccess.com","audience":"a","ownerEmail":"alice@example.com""#;
        let without: AccessConfig = serde_json::from_str(&format!("{{{base}}}")).unwrap();
        assert!(
            !without.allow_scheme_sign_in,
            "a configuration from before keeps the scheme off"
        );
        assert!(!serde_json::to_string(&without)
            .unwrap()
            .contains("allowSchemeSignIn"));
        let with: AccessConfig =
            serde_json::from_str(&format!("{{{base},\"allowSchemeSignIn\":true}}")).unwrap();
        assert!(with.allow_scheme_sign_in);
        assert!(serde_json::to_string(&with)
            .unwrap()
            .contains("\"allowSchemeSignIn\":true"));
    }

    #[test]
    fn a_team_may_name_the_repositories_its_delivery_numbers_come_from() {
        let old = r#"{"org":"acme","team":"app","owner":"owner","runtime":{"command":["scrum","runtime"]}}"#;
        let without: HostedTeam = serde_json::from_str(old).unwrap();
        assert_eq!(
            without.delivery, None,
            "a configuration from before still loads"
        );
        assert!(
            !serde_json::to_string(&without)
                .unwrap()
                .contains("delivery"),
            "and is written without it"
        );

        let with = without.with_delivery(vec!["acme/app".into(), "acme/web".into()]);
        let text = serde_json::to_string(&with).unwrap();
        assert!(
            text.contains(r#""delivery":{"repos":["acme/app","acme/web"]}"#),
            "{text}"
        );
        let again: HostedTeam = serde_json::from_str(&text).unwrap();
        assert_eq!(again.delivery, with.delivery);
        let unknown = text.replace(r#""repos""#, r#""token":"x","repos""#);
        assert!(
            serde_json::from_str::<HostedTeam>(&unknown).is_err(),
            "no other key, a token least of all"
        );
    }

    #[test]
    fn a_team_may_name_the_github_host_its_repositories_are_on() {
        let team = HostedTeam::new(
            Id::parse("acme").unwrap(),
            Id::parse("app").unwrap(),
            Id::parse("owner").unwrap(),
            RuntimeCommand::of(vec!["scrum".into(), "runtime".into()]),
        );
        let on_github = team.clone().with_delivery(vec!["acme/app".into()]);
        let site = on_github.delivery_source().unwrap();
        assert_eq!(site.host, None, "no host is github.com");
        assert_eq!(site.repos, ["acme/app"]);
        assert!(
            !serde_json::to_string(&on_github).unwrap().contains("host"),
            "and is written without one"
        );

        let enterprise = team
            .clone()
            .with_delivery_on("git.example.test", vec!["acme/app".into()]);
        let text = serde_json::to_string(&enterprise).unwrap();
        assert!(
            text.contains(r#""delivery":{"repos":["acme/app"],"host":"git.example.test"}"#),
            "{text}"
        );
        let again: HostedTeam = serde_json::from_str(&text).unwrap();
        assert_eq!(again.delivery, enterprise.delivery);
        assert_eq!(
            again.delivery_source().unwrap().host.as_deref(),
            Some("git.example.test")
        );

        let named = team
            .clone()
            .with_delivery_on("GitHub.com", vec!["acme/app".into()]);
        assert_eq!(
            named.delivery_source().unwrap().host,
            None,
            "github.com named is github.com, in any case"
        );
        let upper = team.with_delivery_on("Git.Example.TEST", vec!["acme/app".into()]);
        assert_eq!(
            upper.delivery_source().unwrap().host.as_deref(),
            Some("git.example.test"),
            "a host is the same in any case, and is kept in lower case"
        );
    }

    #[test]
    fn a_site_is_the_host_and_the_repositories_and_nothing_else() {
        let site = DeliverySite::new(vec!["acme/app".into()]);
        assert_eq!(site.host, None);
        assert_eq!(site.clone().with_host("github.com"), site);
        assert_eq!(site.clone().with_host("GITHUB.COM"), site);
        assert_ne!(site.clone().with_host("git.example.test"), site);
        assert_eq!(
            site.clone().with_host("git.example.test"),
            site.with_host("GIT.example.test")
        );
    }

    #[test]
    fn a_host_is_a_host_name_or_the_node_does_not_start() {
        for good in [
            "github.com",
            "git.example.test",
            "ghe",
            "a-b.c-d.example.test",
            "10.0.0.7",
        ] {
            assert!(is_host_name(good), "{good}");
        }
        for bad in [
            "",
            "-evil",
            "--hostname",
            "git.example.test --jq x",
            "git example",
            "git.example.test/path",
            "https://git.example.test",
            "git.example.test:8443",
            "git..example.test",
            ".example.test",
            "example.test.",
            "ex_ample.test",
            "git.example.test?x=1",
            "-a.example.test",
            "a-.example.test",
        ] {
            assert!(!is_host_name(bad), "{bad:?}");
        }
        assert!(!is_host_name(&"a".repeat(64)), "a label of at most 63");
        assert!(is_host_name(&"a".repeat(63)));
        assert!(
            !is_host_name(&format!("{}.com", "a.".repeat(127))),
            "at most 253 in all"
        );

        let at = "teams[1].delivery";
        let with = |host: Option<&str>| DeliveryConfig {
            repos: vec!["acme/app".into()],
            host: host.map(str::to_owned),
        };
        assert_eq!(with(None).check(at), Ok(()));
        assert_eq!(with(Some("git.example.test")).check(at), Ok(()));
        let error = with(Some("https://git.example.test"))
            .check(at)
            .unwrap_err();
        assert!(
            error.starts_with("teams[1].delivery.host is not a host name"),
            "{error}"
        );
    }

    #[test]
    fn a_team_with_no_repository_has_no_delivery_source() {
        let team = HostedTeam::new(
            Id::parse("acme").unwrap(),
            Id::parse("app").unwrap(),
            Id::parse("owner").unwrap(),
            RuntimeCommand::of(vec!["scrum".into(), "runtime".into()]),
        );
        assert_eq!(team.delivery_source(), None);
        assert!(team
            .clone()
            .with_delivery(vec![])
            .delivery_source()
            .is_none());
        assert!(team
            .with_delivery(vec!["acme/app".into()])
            .delivery_source()
            .is_some());
    }

    #[test]
    fn the_repositories_of_a_team_are_few_written_owner_name_and_none_twice() {
        let delivery = |repos: &[&str]| DeliveryConfig {
            repos: repos.iter().map(|one| (*one).to_owned()).collect(),
            host: None,
        };
        let at = "teams[1].delivery";
        assert_eq!(delivery(&["acme/app", "acme/web"]).check(at), Ok(()));
        assert_eq!(
            delivery(&[]).check(at),
            Ok(()),
            "none is the team having no numbers, which is not this check's"
        );
        for (repos, wanted) in [
            (
                vec!["acme"],
                "teams[1].delivery.repos[0] is not written owner/name",
            ),
            (
                vec!["acme/app", "acme/web/x"],
                "teams[1].delivery.repos[1] is not written owner/name",
            ),
            (
                vec!["acme/app", "Acme/App"],
                "teams[1].delivery.repos[1] names a repository twice",
            ),
            (vec!["acme/app", "acme/app"], "names a repository twice"),
        ] {
            let error = delivery(&repos).check(at).unwrap_err();
            assert!(error.contains(wanted), "{error}");
        }
        let many: Vec<String> = (0..=MAX_REPOSITORIES)
            .map(|n| format!("acme/r{n}"))
            .collect();
        let many = delivery(&many.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(many.check(at).unwrap_err().contains("more than 50"));
        let fifty: Vec<String> = (0..MAX_REPOSITORIES)
            .map(|n| format!("acme/r{n}"))
            .collect();
        let fifty = delivery(&fifty.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(fifty.check(at), Ok(()));
    }

    #[test]
    fn a_web_push_subject_is_a_mailto_or_https_contact() {
        let subject = |text: &str| WebPushConfig {
            subject: Some(text.to_owned()),
            extra_hosts: vec![],
        };
        for good in ["mailto:ops@acme.example", "https://node.acme.example"] {
            assert_eq!(subject(good).check(), Ok(()), "{good}");
        }
        for bad in [
            "",
            "mailto:",
            "ops@acme.example",
            "http://127.0.0.1:4400",
            "https://",
            "mailto:a b@x.example",
        ] {
            let error = subject(bad).check().unwrap_err();
            assert!(error.contains("webPush.subject"), "{bad}: {error}");
        }
    }

    #[test]
    fn web_push_hosts_must_be_names_and_say_which_entry_is_not() {
        let hosts = |list: &[&str]| WebPushConfig {
            subject: None,
            extra_hosts: list.iter().map(|one| (*one).to_owned()).collect(),
        };
        assert_eq!(hosts(&[]).check(), Ok(()));
        assert_eq!(
            hosts(&["push.acme.example", "*.edge.acme.example"]).check(),
            Ok(())
        );
        for bad in [
            "127.0.0.1",
            "0x7f.1",
            "2130706433",
            "*.com",
            "Push.Acme.example",
        ] {
            let error = hosts(&["push.acme.example", bad]).check().unwrap_err();
            assert!(error.contains(bad), "{error}");
        }
    }

    #[test]
    fn a_claude_collector_renews_by_default_and_says_so_in_the_file_only_when_it_does_not() {
        let default: UsageCollector = serde_json::from_str(r#"{"type":"claude"}"#).unwrap();
        assert_eq!(
            default,
            UsageCollector::Claude {
                config_dir: None,
                renew: true,
                command: None
            }
        );
        assert_eq!(
            serde_json::to_string(&default).unwrap(),
            r#"{"type":"claude"}"#,
            "the default is not written"
        );
        let off: UsageCollector = serde_json::from_str(
            r#"{"type":"claude","configDir":"/home/alice/.claude-a","renew":false,"command":"/opt/bin/claude"}"#,
        )
        .unwrap();
        assert_eq!(
            off,
            UsageCollector::Claude {
                config_dir: Some("/home/alice/.claude-a".into()),
                renew: false,
                command: Some("/opt/bin/claude".into())
            }
        );
        assert_eq!(
            serde_json::to_string(&off).unwrap(),
            r#"{"type":"claude","configDir":"/home/alice/.claude-a","renew":false,"command":"/opt/bin/claude"}"#
        );
        assert!(
            serde_json::from_str::<UsageCollector>(r#"{"type":"claude","renews":false}"#).is_err()
        );
        assert!(
            serde_json::from_str::<UsageCollector>(r#"{"type":"codex","renew":false}"#).is_err()
        );
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
