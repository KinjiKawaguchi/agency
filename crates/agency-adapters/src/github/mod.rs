//! GitHub as the source of a team's delivery facts, read through the machine's `gh` (read-only).
mod gh;
mod pulls;
mod runs;

use agency_core::delivery::DeliveryFacts;
use agency_core::ports::{DeliveryError, DeliverySource};
use agency_proto::clock::OffsetDateTime;
use async_trait::async_trait;
use std::sync::Arc;

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

#[async_trait]
impl DeliverySource for GithubDelivery {
    async fn facts(
        &self,
        repos: &[String],
        since: OffsetDateTime,
    ) -> Result<DeliveryFacts, DeliveryError> {
        let mut facts = DeliveryFacts::default();
        for repo in repos {
            let (owner, name) = split_repo(repo).ok_or_else(|| {
                DeliveryError("a repository of the team is not written owner/name".into())
            })?;
            let fail = |error: GhError| DeliveryError(error.reason().into());
            facts.pull_requests.extend(
                pulls::merged_since(self.gh.as_ref(), owner, name, since)
                    .await
                    .map_err(fail)?,
            );
            facts.runs.extend(
                runs::main_runs(self.gh.as_ref(), owner, name, since)
                    .await
                    .map_err(fail)?,
            );
        }
        Ok(facts)
    }
}

#[cfg(test)]
mod tests;
