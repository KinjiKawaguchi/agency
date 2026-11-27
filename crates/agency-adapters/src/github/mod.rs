//! GitHub as the source of a team's delivery facts, read through the machine's `gh` (read-only).
mod cache;
mod gh;
mod pulls;
mod runs;

use agency_core::config::DeliveryConfig;
use agency_core::delivery::DeliveryFacts;
use agency_core::ports::{DeliveryError, DeliverySource};
use agency_proto::clock::OffsetDateTime;
use async_trait::async_trait;
use std::sync::Arc;

pub use cache::CachedDelivery;
pub use gh::{Gh, GhCli, GhError};

pub struct GithubDelivery {
    gh: Arc<dyn Gh>,
}

impl GithubDelivery {
    pub fn new(gh: Arc<dyn Gh>) -> Self {
        Self { gh }
    }
}

impl Default for GithubDelivery {
    fn default() -> Self {
        Self::new(Arc::new(GhCli::default()))
    }
}

/// `owner/name`, where each is a GitHub name. It goes into a request path, so nothing else is let in.
fn split_repo(repo: &str) -> Option<(&str, &str)> {
    let valid = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    let (owner, name) = repo.split_once('/')?;
    (valid(owner) && valid(name)).then_some((owner, name))
}

/// A GitHub host name: dot-separated labels of letters, digits and hyphens, none starting or ending with a
/// hyphen. It goes to `gh` as the value of `--hostname`, so nothing else is let in.
fn is_host_name(host: &str) -> bool {
    let is_label = |label: &str| {
        (1..=63).contains(&label.len())
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    host.len() <= 253 && host.split('.').all(is_label)
}

/// A repository to read: `owner/name` on a host that `gh` is told, unless it is github.com.
pub(super) struct Repo<'a> {
    host: Option<&'a str>,
    owner: &'a str,
    name: &'a str,
}

impl Repo<'_> {
    fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// What tells `gh` which host to ask: nothing for github.com, so that its call is the same as ever.
    fn host_args(&self) -> Vec<String> {
        self.host
            .map(|host| vec!["--hostname".to_owned(), host.to_owned()])
            .unwrap_or_default()
    }
}

#[async_trait]
impl DeliverySource for GithubDelivery {
    async fn facts(
        &self,
        source: &DeliveryConfig,
        since: OffsetDateTime,
    ) -> Result<DeliveryFacts, DeliveryError> {
        let host = source.custom_host();
        if host.is_some_and(|host| !is_host_name(host)) {
            return Err(DeliveryError(
                "the GitHub host the team names is not a host name".into(),
            ));
        }
        let mut facts = DeliveryFacts::default();
        for repo in &source.repos {
            let (owner, name) = split_repo(repo).ok_or_else(|| {
                DeliveryError("a repository of the team is not written owner/name".into())
            })?;
            let repo = Repo { host, owner, name };
            let fail = |error: GhError| DeliveryError(error.reason(host.is_some()).into());
            facts.pull_requests.extend(
                pulls::merged_since(self.gh.as_ref(), &repo, since)
                    .await
                    .map_err(fail)?,
            );
            facts.runs.extend(
                runs::main_runs(self.gh.as_ref(), &repo, since)
                    .await
                    .map_err(fail)?,
            );
        }
        Ok(facts)
    }
}

#[cfg(test)]
mod tests;
