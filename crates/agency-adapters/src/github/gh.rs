//! Running the GitHub CLI. The node reads GitHub with the machine's own `gh` login, read-only, and holds
//! no token itself (`GH_HOST` and the rest of `gh`'s environment are honored as `gh` honors them).
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// Why a `gh` call failed. None of these carries what `gh` printed: that may name accounts or hosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GhError {
    NotInstalled,
    NotLoggedIn,
    RateLimited,
    TimedOut,
    Failed,
}

impl GhError {
    /// A reason a client may be shown.
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotInstalled => "the GitHub CLI (gh) is not installed on the team's node",
            Self::NotLoggedIn => "the GitHub CLI (gh) on the team's node is not logged in",
            Self::RateLimited => "GitHub's rate limit is used up for now",
            Self::TimedOut => "GitHub did not answer in time",
            Self::Failed => "GitHub did not answer the request",
        }
    }

    /// A reason for a team on a GitHub host of its own: it says that it is that host that could not be
    /// read. It never names the host: a reason goes to every client that reads the team, and a company's
    /// host name is not theirs to be told.
    pub fn own_host_reason(self) -> &'static str {
        match self {
            Self::NotInstalled => self.reason(),
            Self::NotLoggedIn => {
                "the GitHub CLI (gh) on the team's node is not logged in to the GitHub host the team names"
            }
            Self::RateLimited => "the rate limit of the GitHub host the team names is used up for now",
            Self::TimedOut => "the GitHub host the team names did not answer in time",
            Self::Failed => "the GitHub host the team names did not answer the request",
        }
    }
}

/// `gh` with some arguments, answering what it printed on its standard output.
#[async_trait]
pub trait Gh: Send + Sync {
    async fn run(&self, args: &[String]) -> Result<String, GhError>;
}

pub struct GhCli {
    program: PathBuf,
    timeout: Duration,
}

impl GhCli {
    pub fn new(program: PathBuf, timeout: Duration) -> Self {
        Self { program, timeout }
    }
}

impl Default for GhCli {
    fn default() -> Self {
        Self::new(PathBuf::from("gh"), Duration::from_secs(90))
    }
}

/// What a failure of `gh` was, from what it wrote to its standard error.
fn classify(stderr: &str) -> GhError {
    let text = stderr.to_ascii_lowercase();
    if text.contains("gh auth login")
        || text.contains("not logged in")
        || text.contains("bad credentials")
    {
        GhError::NotLoggedIn
    } else if text.contains("rate limit") {
        GhError::RateLimited
    } else {
        GhError::Failed
    }
}

#[async_trait]
impl Gh for GhCli {
    async fn run(&self, args: &[String]) -> Result<String, GhError> {
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = tokio::time::timeout(self.timeout, command.output())
            .await
            .map_err(|_| GhError::TimedOut)?
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => GhError::NotInstalled,
                _ => GhError::Failed,
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(classify(&String::from_utf8_lossy(&output.stderr)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_why_gh_failed_from_what_it_wrote() {
        assert_eq!(
            classify("To get started with GitHub CLI, please run:  gh auth login"),
            GhError::NotLoggedIn
        );
        assert_eq!(classify("HTTP 401: Bad credentials"), GhError::NotLoggedIn);
        assert_eq!(
            classify("API rate limit exceeded for user"),
            GhError::RateLimited
        );
        assert_eq!(classify("HTTP 502"), GhError::Failed);
    }

    #[tokio::test]
    async fn a_missing_program_a_failing_one_and_a_slow_one() {
        let gh = |program: &str, wait: Duration| GhCli::new(PathBuf::from(program), wait);
        let none: Vec<String> = vec![];
        assert_eq!(
            gh("/nonexistent/gh", Duration::from_secs(5))
                .run(&none)
                .await,
            Err(GhError::NotInstalled)
        );
        assert_eq!(
            gh("false", Duration::from_secs(5)).run(&none).await,
            Err(GhError::Failed)
        );
        assert_eq!(
            gh("echo", Duration::from_secs(5)).run(&["hi".into()]).await,
            Ok("hi\n".into())
        );
        assert_eq!(
            gh("sleep", Duration::from_millis(200))
                .run(&["5".into()])
                .await,
            Err(GhError::TimedOut)
        );
    }

    #[test]
    fn no_reason_holds_what_gh_printed() {
        for error in [
            GhError::NotInstalled,
            GhError::NotLoggedIn,
            GhError::RateLimited,
            GhError::TimedOut,
            GhError::Failed,
        ] {
            for reason in [error.reason(), error.own_host_reason()] {
                assert!(!reason.contains("http") && !reason.contains('@'));
            }
        }
    }
}
