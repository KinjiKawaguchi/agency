//! The runtimes of the teams this node hosts, called over agency-runtime/1 (docs/protocol.md).
//! The node never reads or writes a runtime's files; it only runs the team's configured command.
//! Tests use an in-memory runtime with the same behavior.
use crate::config::{HostedTeam, RuntimeCommand};
use agency_proto::runtime::{Decided, Describe, RuntimeError};
use agency_proto::view::{AnswerWith, Request, RequestKind};
use agency_proto::{
    clock, EnvelopeKind, Id, RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse,
    TeamView, Timestamp, RUNTIME_VERSION,
};
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(20);

fn unavailable(message: impl Into<String>) -> RuntimeError {
    RuntimeError {
        status: 503,
        message: message.into(),
    }
}

/// A runtime that keeps requests in memory and records the decisions applied to them.
#[derive(Default, Debug)]
pub struct MemoryRuntime {
    /// Requests waiting for an answer: (id, title).
    pub requests: Vec<(String, String)>,
    pub applied: Vec<RuntimeDecision>,
}

impl MemoryRuntime {
    fn view(&self, team: &HostedTeam) -> TeamView {
        let at = Timestamp::parse(&clock::format(clock::now()))
            .unwrap_or_else(|_| unreachable!("clock::format writes RFC 3339 UTC"));
        let requests = self.requests.iter().map(|(id, title)| Request {
            id: id.clone(),
            kind: RequestKind::Question,
            title: title.clone(),
            context: String::new(),
            document: None,
            from: "lead".into(),
            to: None,
            created_at: at.clone(),
            issue: None,
            blocks: vec![],
            options: vec![],
            thread: None,
            answer_with: AnswerWith {
                kind: EnvelopeKind::Answer,
                target: id.clone(),
            },
        });
        let requests = requests.collect();
        TeamView {
            org: team.org.clone(),
            team: team.team.clone(),
            label: team.team.to_string(),
            generated_at: at,
            people: vec![],
            requests,
            work: vec![],
            epics: vec![],
        }
    }

    fn decide(&mut self, decision: RuntimeDecision) -> Result<(), RuntimeError> {
        if !self.requests.iter().any(|(id, _)| id == &decision.target) {
            return Err(RuntimeError {
                status: 404,
                message: format!("no such request: {}", decision.target),
            });
        }
        self.applied.push(decision);
        Ok(())
    }
}

#[derive(Clone)]
pub enum Runtimes {
    /// Runs each hosted team's command. Remembers which teams already described agency-runtime/1.
    Command {
        described: Arc<Mutex<HashSet<(Id, Id)>>>,
    },
    Memory(Arc<Mutex<MemoryRuntime>>),
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

impl Runtimes {
    pub fn command() -> Self {
        Self::Command {
            described: Arc::default(),
        }
    }

    pub fn memory() -> (Self, Arc<Mutex<MemoryRuntime>>) {
        let inner = Arc::new(Mutex::new(MemoryRuntime::default()));
        (Self::Memory(inner.clone()), inner)
    }

    /// Calls `describe` once per team and refuses runtimes that do not speak agency-runtime/1.
    async fn ensure_described(
        described: &Mutex<HashSet<(Id, Id)>>,
        team: &HostedTeam,
    ) -> Result<(), RuntimeError> {
        let key = (team.org.clone(), team.team.clone());
        if described
            .lock()
            .map_err(|_| unavailable("runtime state is poisoned"))?
            .contains(&key)
        {
            return Ok(());
        }
        let description: Describe = call(
            &team.runtime,
            &RuntimeRequest::new(team.org.clone(), team.team.clone(), RuntimeCall::Describe),
        )
        .await?;
        if !description.speaks(RUNTIME_VERSION) {
            return Err(unavailable(format!(
                "the team runtime does not speak {RUNTIME_VERSION}"
            )));
        }
        described
            .lock()
            .map_err(|_| unavailable("runtime state is poisoned"))?
            .insert(key);
        Ok(())
    }

    /// The team's TeamView. A view of another team is refused.
    pub async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        let view = match self {
            Self::Command { described } => {
                Self::ensure_described(described, team).await?;
                call(
                    &team.runtime,
                    &RuntimeRequest::new(team.org.clone(), team.team.clone(), RuntimeCall::View),
                )
                .await?
            }
            Self::Memory(inner) => inner
                .lock()
                .map_err(|_| unavailable("the memory runtime is poisoned"))?
                .view(team),
        };
        if view.org != team.org || view.team != team.team {
            return Err(unavailable(format!(
                "the team runtime reported {}/{} instead of {}/{}",
                view.org, view.team, team.org, team.team
            )));
        }
        Ok(view)
    }

    /// Applies a verified decision.
    pub async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Command { described } => {
                Self::ensure_described(described, team).await?;
                let _: Decided = call(
                    &team.runtime,
                    &RuntimeRequest::new(
                        team.org.clone(),
                        team.team.clone(),
                        RuntimeCall::Decide { decision },
                    ),
                )
                .await?;
                Ok(())
            }
            Self::Memory(inner) => inner
                .lock()
                .map_err(|_| unavailable("the memory runtime is poisoned"))?
                .decide(decision),
        }
    }
}
