//! A team's delivery setting: the GitHub repositories its numbers come from, the host they are on, and the board
//! field that tells a shared repository's teams apart.
use agency_proto::evaluation::{is_repository, MAX_REPOSITORIES};
use serde::{Deserialize, Serialize};

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
    /// For a team that shares its repositories with other teams: the board's single-select field that says
    /// which team an issue belongs to (`Team`), and this team's option of it (`Keel`). Only the pull requests
    /// that close an issue with this option count for the team. Both or neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_value: Option<String>,
}

/// The longest a board field's name or option may be, in characters.
const MOST_BOARD_NAME: usize = 80;

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
        if self.host.as_deref().is_some_and(|host| !is_host_name(host)) {
            return Err(format!(
                "{at}.host is not a host name (letters, digits, hyphens and dots; no scheme, port or path)"
            ));
        }
        let named = |what: &str, text: &Option<String>| -> Result<(), String> {
            let Some(text) = text else { return Ok(()) };
            let is_bad = text.trim().is_empty()
                || text.chars().count() > MOST_BOARD_NAME
                || text.chars().any(char::is_control);
            if is_bad {
                return Err(format!(
                    "{at}.{what} is empty, longer than {MOST_BOARD_NAME} characters, or holds a control character"
                ));
            }
            Ok(())
        };
        named("teamField", &self.team_field)?;
        named("teamValue", &self.team_value)?;
        if self.team_field.is_some() != self.team_value.is_some() {
            return Err(format!(
                "{at} sets only one of teamField and teamValue; set both, or neither"
            ));
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HostedTeam, RuntimeCommand};
    use agency_proto::Id;

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
    fn a_team_of_a_shared_repository_names_the_boards_field_and_its_own_option_or_neither() {
        let team = HostedTeam::new(
            Id::parse("acme").unwrap(),
            Id::parse("app").unwrap(),
            Id::parse("alice").unwrap(),
            RuntimeCommand::of(vec!["scrum".into()]),
        );
        assert_eq!(
            team.clone().with_team_value("Team", "Keel").delivery,
            None,
            "a team without delivery has no repositories to share"
        );
        let shared = team
            .with_delivery(vec!["acme/app".into()])
            .with_team_value("Team", "Keel");
        let text = serde_json::to_string(&shared).unwrap();
        assert!(
            text.contains(
                r#""delivery":{"repos":["acme/app"],"teamField":"Team","teamValue":"Keel"}"#
            ),
            "{text}"
        );
        assert_eq!(
            serde_json::from_str::<HostedTeam>(&text).unwrap().delivery,
            shared.delivery
        );
        let delivery = |field: Option<&str>, value: Option<&str>| DeliveryConfig {
            repos: vec!["acme/app".into()],
            host: None,
            team_field: field.map(str::to_owned),
            team_value: value.map(str::to_owned),
        };
        let at = "teams[2].delivery";
        assert_eq!(delivery(None, None).check(at), Ok(()));
        assert_eq!(delivery(Some("Team"), Some("Keel")).check(at), Ok(()));
        for (field, value, wanted) in [
            (
                Some("Team"),
                None,
                "sets only one of teamField and teamValue",
            ),
            (
                None,
                Some("Keel"),
                "sets only one of teamField and teamValue",
            ),
            (
                Some(" "),
                Some("Keel"),
                "teams[2].delivery.teamField is empty",
            ),
            (
                Some("Team"),
                Some(""),
                "teams[2].delivery.teamValue is empty",
            ),
            (
                Some("Te\nam"),
                Some("Keel"),
                "teams[2].delivery.teamField is empty",
            ),
        ] {
            let error = delivery(field, value).check(at).unwrap_err();
            assert!(error.contains(wanted), "{error}");
        }
        let long = "x".repeat(81);
        assert!(delivery(Some(&long), Some("Keel")).check(at).is_err());
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
            team_field: None,
            team_value: None,
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
            team_field: None,
            team_value: None,
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
}
