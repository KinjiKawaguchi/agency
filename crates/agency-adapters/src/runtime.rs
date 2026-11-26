//! Team runtimes run as commands over agency-runtime/1 (docs/protocol.md). The node never reads or writes
//! a runtime's files; it runs the team's configured command once per call, without a shell.
use crate::activity_window::Window;
use crate::Log;
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::ports::TeamRuntime;
use agency_core::views::check_view;
use agency_proto::activity::{Activity, MAX_LIMIT};
use agency_proto::clock;
use agency_proto::runtime::{Decided, Describe, RuntimeError, ACTIVITY_OP, TOKEN_USAGE_OP};
use agency_proto::{
    Id, RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse, TeamView, Timestamp,
    TokenUsage, RUNTIME_VERSION,
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// How long a team's view is reused. Every page, push pass, decision check, and signed read from
/// another node asks for the view, and each run of the command is a new process.
pub const VIEW_KEPT: Duration = Duration::from_secs(5);
/// How long a team's token usage is reused; it changes slowly.
pub const USAGE_KEPT: Duration = Duration::from_secs(60);
/// How long a team's activity read is reused. Every open activity page reads every team of the org
/// every few seconds, and a read that cannot share one run starts a process each.
pub const ACTIVITY_KEPT: Duration = Duration::from_secs(10);
/// A shared activity read starts this long before the first reader's `since`, so readers whose `since`
/// differ a little (the default is an hour before each read) can share it.
const ACTIVITY_MARGIN_MINUTES: i64 = 5;

type Team = (Id, Id);
type Slot<T> = Arc<tokio::sync::Mutex<Option<(Instant, u32, Result<T, RuntimeError>)>>>;

/// One run of a team's command at a time per kind of call, whose answer is reused for a while.
/// Callers that arrive while it runs wait for it and share its answer, so readers, however many,
/// start at most one process per team and kind at a time.
struct Shared<T> {
    slots: Mutex<HashMap<Team, Slot<T>>>,
    kept: Duration,
}

impl<T: Clone> Shared<T> {
    fn new(kept: Duration) -> Self {
        Self {
            slots: Mutex::default(),
            kept,
        }
    }

    fn slot(&self, team: &HostedTeam) -> Slot<T> {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = (team.org.clone(), team.team.clone());
        slots.entry(key).or_default().clone()
    }

    /// The answer for `team` and `variant` (the range of a usage read; 0 otherwise): the one kept, if
    /// it is recent and for the same variant, or a new one from `run`.
    async fn get(
        &self,
        team: &HostedTeam,
        variant: u32,
        run: impl std::future::Future<Output = Result<T, RuntimeError>>,
    ) -> Result<T, RuntimeError> {
        let slot = self.slot(team);
        let mut kept = slot.lock().await;
        if let Some((at, kept_variant, answer)) = kept.as_ref() {
            if *kept_variant == variant && at.elapsed() < self.kept {
                return answer.clone();
            }
        }
        let answer = run.await;
        *kept = Some((Instant::now(), variant, answer.clone()));
        answer
    }

    /// Drops what is kept for `team`, so the next call runs the command.
    async fn forget(&self, team: &HostedTeam) {
        *self.slot(team).lock().await = None;
    }
}

/// Runs each hosted team's command. Remembers the description of each team that described
/// agency-runtime/1, shares and reuses views and usage (see `Shared`), and logs when a team's runtime
/// starts failing, fails differently, or answers again.
pub struct CommandRuntime {
    described: Mutex<HashMap<(Id, Id), Describe>>,
    views: Shared<TeamView>,
    usage: Shared<TokenUsage>,
    /// A window of a team's activity, cut for each reader (see `Window`).
    activity: Shared<Window>,
    /// Each team's last failure, or `None` after it last answered. Teams not called yet are absent.
    health: Mutex<HashMap<(Id, Id), Option<String>>>,
    log: Log,
}

impl Default for CommandRuntime {
    fn default() -> Self {
        Self::with_log(Box::new(|line| eprintln!("{line}")))
    }
}

fn request(team: &HostedTeam, call: RuntimeCall) -> RuntimeRequest {
    RuntimeRequest::new(team.org.clone(), team.team.clone(), call)
}

impl CommandRuntime {
    pub fn with_log(log: Log) -> Self {
        Self {
            described: Mutex::default(),
            views: Shared::new(VIEW_KEPT),
            usage: Shared::new(USAGE_KEPT),
            activity: Shared::new(ACTIVITY_KEPT),
            health: Mutex::default(),
            log,
        }
    }

    /// How long views and usage are reused (`VIEW_KEPT` and `USAGE_KEPT` unless set here).
    pub fn keeping(self, view: Duration, usage: Duration) -> Self {
        Self {
            views: Shared::new(view),
            usage: Shared::new(usage),
            ..self
        }
    }

    /// One run of the team's `activity` op, if the runtime lists it: 404 without running the command
    /// otherwise.
    async fn read_activity(
        &self,
        team: &HostedTeam,
        since: &Timestamp,
        limit: u32,
    ) -> Result<Activity, RuntimeError> {
        let run = async {
            if !self.ensure_described(team).await?.supports(ACTIVITY_OP) {
                return Err(RuntimeError {
                    status: 404,
                    message: "the team runtime does not report activity".into(),
                });
            }
            let call_with = RuntimeCall::Activity {
                since: since.clone(),
                limit,
            };
            call(&team.runtime, &request(team, call_with)).await
        };
        let result = run.await;
        self.observe(team, &result);
        result
    }

    /// How long a team's activity read is reused (`ACTIVITY_KEPT` unless set here).
    pub fn keeping_activity(self, activity: Duration) -> Self {
        Self {
            activity: Shared::new(activity),
            ..self
        }
    }

    /// Notes how a call went and logs a change: a failure (the command could not run, did not answer
    /// in time, or answered 5xx) that is new or differs from the last, or the first answer after one.
    /// A runtime's 4xx is an answer: it refuses a request, it does not fail. Every view, decision, and
    /// usage read goes through here, so the log says when a team stops working without a line per call.
    fn observe<T>(&self, team: &HostedTeam, result: &Result<T, RuntimeError>) {
        let failure = match result {
            Err(error) if error.status >= 500 => Some(error.message.clone()),
            _ => None,
        };
        let Ok(mut health) = self.health.lock() else {
            return;
        };
        let key = (team.org.clone(), team.team.clone());
        let before = health.insert(key, failure.clone());
        let name = format!("{}/{}", team.org, team.team);
        match (before, failure) {
            (Some(Some(was)), Some(now)) if was == now => {}
            (_, Some(now)) => (self.log)(&format!(
                "agency: team {name}: the team runtime fails: {now}"
            )),
            (Some(Some(_)), None) => (self.log)(&format!(
                "agency: team {name}: the team runtime answers again"
            )),
            _ => {}
        }
    }

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
        let run = async {
            self.ensure_described(team).await?;
            let view: TeamView = call(&team.runtime, &request(team, RuntimeCall::View)).await?;
            check_view(view, &team.org, &team.team)
                .map_err(|message| unavailable(format!("the team runtime {message}")))
        };
        let result = self.views.get(team, 0, run).await;
        self.observe(team, &result);
        result
    }

    async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        let result = async {
            self.ensure_described(team).await?;
            let _: Decided = call(
                &team.runtime,
                &request(team, RuntimeCall::Decide { decision }),
            )
            .await?;
            Ok(())
        }
        .await;
        self.views.forget(team).await;
        self.observe(team, &result);
        result
    }

    /// Called only if the runtime listed `tokenUsage` in `describe`; otherwise 404.
    async fn token_usage(&self, team: &HostedTeam, days: u32) -> Result<TokenUsage, RuntimeError> {
        let run = async {
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
        };
        let result = self.usage.get(team, days, run).await;
        self.observe(team, &result);
        result
    }

    /// Called only if the runtime listed `activity` in `describe`; otherwise 404. Readers share one run
    /// of the command per team for a short time, each cut to its own `since` and `limit`.
    async fn activity(
        &self,
        team: &HostedTeam,
        since: &Timestamp,
        limit: u32,
    ) -> Result<Activity, RuntimeError> {
        let Some(wanted) = clock::parse(since.as_str()) else {
            return self.read_activity(team, since, limit).await;
        };
        let start = wanted - clock::Duration::minutes(ACTIVITY_MARGIN_MINUTES);
        let run = async {
            let shared = Timestamp::parse(&clock::format(start)).map_err(|error| RuntimeError {
                status: 500,
                message: error.to_string(),
            })?;
            let activity = self.read_activity(team, &shared, MAX_LIMIT).await?;
            Ok(Window {
                since: start,
                activity,
            })
        };
        let window = self.activity.get(team, 0, run).await?;
        match window.answer(since, limit) {
            Some(answer) => Ok(answer),
            None => self.read_activity(team, since, limit).await,
        }
    }
}
