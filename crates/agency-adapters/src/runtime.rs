//! Team runtimes run as commands over agency-runtime/1 (docs/protocol.md). The node never reads or writes
//! a runtime's files; it runs the team's configured command once per call, without a shell.
use crate::activity_window::Window;
use crate::metrics::{Recorder, SeriesKey};
use crate::runtime_releases::ReleaseLog;
use crate::swr::{ReadCounts, Run, Swr};
use crate::Log;
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::ports::TeamRuntime;
use agency_core::views::check_view;
use agency_proto::activity::{Activity, MAX_LIMIT};
use agency_proto::clock;
use agency_proto::metrics::SeriesKind;
use agency_proto::participant_metrics::ParticipantMetrics;
use agency_proto::runtime::{
    Decided, Describe, RuntimeError, ACTIVITY_OP, PARTICIPANT_METRICS_OP, TOKEN_USAGE_OP,
};
use agency_proto::{
    Id, RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse, TeamView, Timestamp,
    TokenUsage, RUNTIME_VERSION,
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long one run of a team's runtime command is waited for.
pub const TIMEOUT: Duration = Duration::from_secs(20);
const TIMEOUT_MESSAGE: &str = "the team runtime did not answer within 20 seconds";

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
        .map_err(|_| unavailable(TIMEOUT_MESSAGE))?
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

/// Whether a failure says that a command was not found: the runtime's own command, or one that it runs
/// (Node says `spawnSync gh ENOENT`, a shell says `command not found`). The usual cause is a `PATH` of
/// the launchd agent or the systemd unit that lacks the command's directory.
fn names_a_missing_command(failure: &str) -> bool {
    ["ENOENT", "No such file or directory", "command not found"]
        .iter()
        .any(|sign| failure.contains(sign))
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
}

/// How long an answer that is old but not too old is still served at once, while a refresh runs in the
/// background. A read waits for the command only with nothing younger in memory (the first read of a team,
/// or after the runtime has been failing for this long).
pub const MAX_STALE: Duration = Duration::from_secs(60);

/// What a runtime keeps and shares between calls. `CommandRuntime` holds it in an `Arc` so that a refresh in
/// the background can own it.
struct Inner {
    described: Mutex<HashMap<(Id, Id), Describe>>,
    views: Swr<TeamView>,
    usage: Shared<TokenUsage>,
    /// Participant metrics, kept for each range and time zone like usage.
    participants: Shared<ParticipantMetrics>,
    /// A window of a team's activity, cut for each reader (see `Window`).
    activity: Swr<Window>,
    /// Each team's last failure, or `None` after it last answered. Teams not called yet are absent.
    health: Mutex<HashMap<(Id, Id), Option<String>>>,
    log: Log,
    /// Where the calls to the team's command are timed (the `call` series `runtime decide`).
    recorder: Option<Recorder>,
    /// Where the release each runtime says it is, in `describe`, is noted for `agency status`.
    releases: Option<ReleaseLog>,
}

/// Runs each hosted team's command. Remembers the description of each team that described
/// agency-runtime/1, serves views and activity from memory and refreshes them in the background (see
/// `Swr`), shares and reuses usage (see `Shared`), and logs when a team's runtime starts failing, fails
/// differently, or answers again.
pub struct CommandRuntime {
    inner: Arc<Inner>,
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
        let inner = Inner {
            described: Mutex::default(),
            views: Swr::new(VIEW_KEPT, MAX_STALE),
            usage: Shared::new(USAGE_KEPT),
            participants: Shared::new(USAGE_KEPT),
            activity: Swr::new(ACTIVITY_KEPT, MAX_STALE),
            health: Mutex::default(),
            log,
            recorder: None,
            releases: None,
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// The state, for a setting made before the runtime is used (nothing else holds it yet).
    fn configuring(&mut self) -> &mut Inner {
        Arc::get_mut(&mut self.inner).expect("a runtime is set up before it is used")
    }

    /// Times each delivery of a decision to the team's command into `recorder`, as the `call` series
    /// `runtime decide`: `ok` when the runtime answered (a 4xx refusal is an answer), `timeout` when it
    /// did not answer in time, `error` otherwise.
    pub fn recording(mut self, recorder: Recorder) -> Self {
        self.configuring().recorder = Some(recorder);
        self
    }

    /// Notes into `releases` the release that each team's runtime says it is when `describe` is read.
    pub fn reporting_releases(mut self, releases: ReleaseLog) -> Self {
        self.configuring().releases = Some(releases);
        self
    }

    /// How long a view is served as it is, and how long usage is reused (`VIEW_KEPT` and `USAGE_KEPT`
    /// unless set here).
    pub fn keeping(mut self, view: Duration, usage: Duration) -> Self {
        let inner = self.configuring();
        inner.views = Swr::new(view, MAX_STALE);
        inner.usage = Shared::new(usage);
        inner.participants = Shared::new(usage);
        self
    }

    /// How long an activity read is served as it is (`ACTIVITY_KEPT` unless set here).
    pub fn keeping_activity(mut self, activity: Duration) -> Self {
        self.configuring().activity = Swr::new(activity, MAX_STALE);
        self
    }

    /// How long an old answer is still served while it is refreshed (`MAX_STALE` unless set here).
    pub fn serving_stale_for(mut self, max_stale: Duration) -> Self {
        let inner = self.configuring();
        inner.views = Swr::new(inner.views.fresh(), max_stale);
        inner.activity = Swr::new(inner.activity.fresh(), max_stale);
        self
    }

    /// How many view reads were served young, old (and refreshed), and cold (they waited for a run).
    pub fn view_reads(&self) -> ReadCounts {
        self.inner.views.counts()
    }

    /// The same for activity reads.
    pub fn activity_reads(&self) -> ReadCounts {
        self.inner.activity.counts()
    }
}

impl Inner {
    fn record_call<T>(&self, name: &str, started: Instant, result: &Result<T, RuntimeError>) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let status = match result {
            Ok(_) => "ok",
            Err(error) if error.message == TIMEOUT_MESSAGE => "timeout",
            Err(error) if error.status >= 500 => "error",
            Err(_) => "ok",
        };
        recorder.record(
            SeriesKey::new(SeriesKind::Call, name, status),
            started.elapsed(),
        );
    }

    /// One run of the team's view, checked.
    async fn run_view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        let result = async {
            self.ensure_described(team).await?;
            let view: TeamView = call(&team.runtime, &request(team, RuntimeCall::View)).await?;
            check_view(view, &team.org, &team.team)
                .map_err(|message| unavailable(format!("the team runtime {message}")))
        }
        .await;
        self.observe(team, &result);
        result
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
            (_, Some(now)) => {
                (self.log)(&format!(
                    "agency: team {name}: the team runtime fails: {now}"
                ));
                if names_a_missing_command(&now) {
                    (self.log)(&format!(
                        "agency: team {name}: a command the team runtime runs was not found: put its directory on the PATH of the launchd agent or the systemd unit (docs/install.md, \"Run it with launchd\")"
                    ));
                }
            }
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
        if let Some(releases) = &self.releases {
            releases.note(&team.org, &team.team, &description);
        }
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

fn view_run(inner: &Arc<Inner>, team: &HostedTeam) -> Run<TeamView> {
    let (inner, team) = (inner.clone(), team.clone());
    Arc::new(move || {
        let (inner, team) = (inner.clone(), team.clone());
        Box::pin(async move { inner.run_view(&team).await })
    })
}

/// A run of the window of a team's activity that starts at `start`, at the largest limit.
fn window_run(inner: &Arc<Inner>, team: &HostedTeam, start: clock::OffsetDateTime) -> Run<Window> {
    let (inner, team) = (inner.clone(), team.clone());
    Arc::new(move || {
        let (inner, team) = (inner.clone(), team.clone());
        Box::pin(async move {
            let since = Timestamp::parse(&clock::format(start)).map_err(|error| RuntimeError {
                status: 500,
                message: error.to_string(),
            })?;
            let activity = inner.read_activity(&team, &since, MAX_LIMIT).await?;
            Ok(Window {
                since: start,
                activity,
            })
        })
    })
}

#[async_trait]
impl TeamRuntime for CommandRuntime {
    /// The team's TeamView, from memory when there is a usable one (see `Swr`). A view of another team is
    /// refused.
    async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        self.inner
            .views
            .get(team, view_run(&self.inner, team))
            .await
    }

    async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        let started = Instant::now();
        let result = async {
            self.inner.ensure_described(team).await?;
            let _: Decided = call(
                &team.runtime,
                &request(team, RuntimeCall::Decide { decision }),
            )
            .await?;
            Ok(())
        }
        .await;
        self.inner
            .views
            .invalidate_and_refresh(team, view_run(&self.inner, team));
        self.inner.observe(team, &result);
        self.inner.record_call("runtime decide", started, &result);
        result
    }

    /// Called only if the runtime listed `tokenUsage` in `describe`; otherwise 404.
    async fn token_usage(&self, team: &HostedTeam, days: u32) -> Result<TokenUsage, RuntimeError> {
        let run = async {
            if !self
                .inner
                .ensure_described(team)
                .await?
                .supports(TOKEN_USAGE_OP)
            {
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
        let result = self.inner.usage.get(team, days, run).await;
        self.inner.observe(team, &result);
        result
    }

    /// Called only if the runtime listed `participantMetrics` in `describe`; otherwise 404. Kept for each
    /// range and time zone, like usage.
    async fn participant_metrics(
        &self,
        team: &HostedTeam,
        days: u32,
        utc_offset_minutes: i32,
    ) -> Result<ParticipantMetrics, RuntimeError> {
        let run = async {
            if !self
                .inner
                .ensure_described(team)
                .await?
                .supports(PARTICIPANT_METRICS_OP)
            {
                return Err(RuntimeError {
                    status: 404,
                    message: "the team runtime does not report participant metrics".into(),
                });
            }
            let asked = RuntimeCall::ParticipantMetrics {
                days,
                utc_offset_minutes: Some(utc_offset_minutes),
            };
            call(&team.runtime, &request(team, asked)).await
        };
        // The range and the time zone make one key: a time zone is at most 1560 minutes wide.
        let variant = days * 2048 + u32::try_from(utc_offset_minutes + 720).unwrap_or(0);
        let result = self.inner.participants.get(team, variant, run).await;
        self.inner.observe(team, &result);
        result
    }

    /// Called only if the runtime listed `activity` in `describe`; otherwise 404. Readers share one window
    /// of the team's activity, served from memory and refreshed in the background, each cut to its own
    /// `since` and `limit`.
    async fn activity(
        &self,
        team: &HostedTeam,
        since: &Timestamp,
        limit: u32,
    ) -> Result<Activity, RuntimeError> {
        let Some(wanted) = clock::parse(since.as_str()) else {
            return self.inner.read_activity(team, since, limit).await;
        };
        let start = wanted - clock::Duration::minutes(ACTIVITY_MARGIN_MINUTES);
        let window = self
            .inner
            .activity
            .get(team, window_run(&self.inner, team, start))
            .await?;
        match window.answer(since, limit) {
            Some(answer) => Ok(answer),
            None => self.inner.read_activity(team, since, limit).await,
        }
    }
}
