//! Team runtimes run as commands over agency-runtime/1 (docs/protocol.md). The node never reads or writes
//! a runtime's files; it runs the team's configured command once per call, without a shell.
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::ports::TeamRuntime;
use agency_core::views::check_view;
use agency_proto::runtime::{Decided, Describe, RuntimeError, TOKEN_USAGE_OP};
use agency_proto::{
    Id, RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse, TeamView, TokenUsage,
    RUNTIME_VERSION,
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(20);

fn unavailable(message: impl Into<String>) -> RuntimeError {
    RuntimeError {
        status: 503,
        message: message.into(),
    }
}

/// Runs the command once: one request on stdin, one response on stdout.
async fn call<T: DeserializeOwned>(
    runtime: &RuntimeCommand,
    request: &RuntimeRequest,
) -> Result<T, RuntimeError> {
    use tokio::io::AsyncWriteExt;
    let (program, args) = runtime
        .command
        .split_first()
        .ok_or_else(|| unavailable("the team runtime has no command"))?;
    let input = serde_json::to_vec(request).map_err(|error| unavailable(error.to_string()))?;
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .envs(&runtime.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| unavailable(format!("cannot start the team runtime: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&input)
            .await
            .map_err(|error| unavailable(format!("cannot write to the team runtime: {error}")))?;
    }
    let output = tokio::time::timeout(TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| unavailable("the team runtime did not answer within 20 seconds"))?
        .map_err(|error| unavailable(format!("the team runtime failed: {error}")))?;
    match serde_json::from_slice::<RuntimeResponse<T>>(&output.stdout) {
        Ok(RuntimeResponse::Result(result)) => Ok(result),
        Ok(RuntimeResponse::Error(error)) => Err(error),
        Err(_) => {
            let stderr: String = String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(300)
                .collect();
            Err(unavailable(format!(
                "the team runtime did not write an agency-runtime/1 response ({}): {stderr}",
                output.status
            )))
        }
    }
}

/// Runs each hosted team's command. Remembers the description of each team that described
/// agency-runtime/1.
#[derive(Default)]
pub struct CommandRuntime {
    described: Mutex<HashMap<(Id, Id), Describe>>,
}

fn request(team: &HostedTeam, call: RuntimeCall) -> RuntimeRequest {
    RuntimeRequest::new(team.org.clone(), team.team.clone(), call)
}

impl CommandRuntime {
    /// Calls `describe` once per team, refuses runtimes that do not speak agency-runtime/1, and
    /// returns its description.
    async fn ensure_described(&self, team: &HostedTeam) -> Result<Describe, RuntimeError> {
        let key = (team.org.clone(), team.team.clone());
        let known = self
            .described
            .lock()
            .map_err(|_| unavailable("runtime state is poisoned"))?
            .get(&key)
            .cloned();
        if let Some(description) = known {
            return Ok(description);
        }
        let description: Describe =
            call(&team.runtime, &request(team, RuntimeCall::Describe)).await?;
        if !description.speaks(RUNTIME_VERSION) {
            return Err(unavailable(format!(
                "the team runtime does not speak {RUNTIME_VERSION}"
            )));
        }
        self.described
            .lock()
            .map_err(|_| unavailable("runtime state is poisoned"))?
            .insert(key, description.clone());
        Ok(description)
    }
}

#[async_trait]
impl TeamRuntime for CommandRuntime {
    /// The team's TeamView. A view of another team is refused.
    async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        self.ensure_described(team).await?;
        let view: TeamView = call(&team.runtime, &request(team, RuntimeCall::View)).await?;
        check_view(view, &team.org, &team.team)
            .map_err(|message| unavailable(format!("the team runtime {message}")))
    }

    async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        self.ensure_described(team).await?;
        let _: Decided = call(
            &team.runtime,
            &request(team, RuntimeCall::Decide { decision }),
        )
        .await?;
        Ok(())
    }

    /// Called only if the runtime listed `tokenUsage` in `describe`; otherwise 404.
    async fn token_usage(&self, team: &HostedTeam, days: u32) -> Result<TokenUsage, RuntimeError> {
        if !self.ensure_described(team).await?.supports(TOKEN_USAGE_OP) {
            return Err(RuntimeError {
                status: 404,
                message: "the team runtime does not report token usage".into(),
            });
        }
        call(
            &team.runtime,
            &request(team, RuntimeCall::TokenUsage { days }),
        )
        .await
    }
}
