//! Renewing a login that nobody uses. An access token of a Claude Code login runs out after hours, and it
//! renews only when Claude Code runs with the login: a login no session uses stays expired, its usage is
//! unknown, and nothing is ever moved onto it.
//!
//! The node never renews a token itself. A refresh token is used once and replaced by a new one, and two
//! programs that refresh the same login at once leave one of them with a token that no longer works, which
//! Claude Code answers by forgetting the login; the node cannot take Claude Code's lock either. So the
//! node runs `claude` once, with a tiny request, and lets it renew the login the way it does for a person.
//! That is bounded: only a token that has been expired for a while (a running session would have renewed
//! it by then), at most one run in 3 hours and 4 in a day for a login, and a pause of 6 hours after a
//! run that failed. What the run prints is thrown away and never logged.

use super::schedule::Failure;
use super::usage_file;
use super::Log;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

/// How long a token has to have been expired before the node runs `claude`.
pub const GRACE_SECONDS: i64 = 10 * 60;
/// The least time between two runs for one login.
pub const MIN_GAP_SECONDS: i64 = 3 * 3600;
/// The most runs in 24 hours for one login.
pub const MAX_PER_DAY: usize = 4;
/// The pause after a run that failed.
pub const AFTER_FAILURE_SECONDS: i64 = 6 * 3600;
const DAY_SECONDS: i64 = 24 * 3600;
/// How long a run may take.
pub const RUN_LIMIT: Duration = Duration::from_secs(60);

/// The runs made for one login: when, and when the last one failed. Kept in a file, so a node that
/// restarts does not start counting again.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// Unix seconds of the runs in the last 24 hours.
    #[serde(default)]
    pub runs: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<i64>,
}

impl Record {
    /// Whether a run may start at `now` (Unix seconds); if not, in how many seconds it may.
    pub fn may_run_at(&self, now: i64) -> Result<(), i64> {
        let mut newest_first = self.runs.clone();
        newest_first.sort_unstable_by(|a, b| b.cmp(a));
        let mut waits = vec![];
        if let Some(failed) = self.failed_at {
            waits.push(failed + AFTER_FAILURE_SECONDS - now);
        }
        if let Some(last) = newest_first.first() {
            waits.push(last + MIN_GAP_SECONDS - now);
        }
        // The fourth newest run is the one whose day has to pass before there is room for another.
        if let Some(fourth) = newest_first.get(MAX_PER_DAY - 1) {
            waits.push(fourth + DAY_SECONDS - now);
        }
        match waits.into_iter().max() {
            Some(wait) if wait > 0 => Err(wait),
            _ => Ok(()),
        }
    }

    /// The record after a run at `now` that succeeded or failed.
    pub fn after_run(&self, now: i64, is_success: bool) -> Self {
        let mut runs: Vec<i64> = self
            .runs
            .iter()
            .copied()
            .filter(|run| now - run < DAY_SECONDS)
            .collect();
        runs.push(now);
        Self {
            runs,
            failed_at: (!is_success).then_some(now),
        }
    }
}

/// Why a run of `claude` did not renew the login. The text says nothing of what `claude` printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunFailure {
    /// `claude` could not be started.
    NotStarted(String),
    TimedOut,
    /// It exited with an error: usually the login was refused and needs a new `/login`.
    Exited(Option<i32>),
}

impl fmt::Display for RunFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotStarted(why) => write!(
                f,
                "claude could not be started ({why}); put its directory on the PATH the node runs with, or set the collector's command"
            ),
            Self::TimedOut => write!(f, "claude did not finish within {} seconds", RUN_LIMIT.as_secs()),
            Self::Exited(Some(status)) => write!(
                f,
                "claude exited with status {status}; if the login was refused, log in again with `claude` and `/login` for that login"
            ),
            Self::Exited(None) => write!(f, "claude was stopped by a signal"),
        }
    }
}

/// Runs `claude` so that it renews the login. Replaced in tests.
#[async_trait]
pub trait RunClaude: Send + Sync {
    async fn run(&self) -> Result<(), RunFailure>;
}

/// The real `claude`: a tiny request in print mode, with the login's `CLAUDE_CONFIG_DIR` and nothing that
/// would make it use another credential.
pub struct ClaudeProgram {
    pub command: PathBuf,
    pub config_dir: Option<PathBuf>,
    /// Where it runs: an empty directory, so no project file of anyone's reaches it.
    pub work_dir: PathBuf,
    pub limit: Duration,
    /// Arguments before the request's own, for a test that runs a script as `sh <script> -p …`.
    pub leading_args: Vec<PathBuf>,
}

impl ClaudeProgram {
    pub fn new(command: PathBuf, config_dir: Option<PathBuf>, work_dir: PathBuf) -> Self {
        Self {
            command,
            config_dir,
            work_dir,
            limit: RUN_LIMIT,
            leading_args: vec![],
        }
    }
}

/// Credentials from the environment that `claude` would use instead of the login.
const OTHER_CREDENTIALS: [&str; 3] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
];

#[async_trait]
impl RunClaude for ClaudeProgram {
    async fn run(&self) -> Result<(), RunFailure> {
        std::fs::create_dir_all(&self.work_dir)
            .map_err(|error| RunFailure::NotStarted(error.kind().to_string()))?;
        let mut command = tokio::process::Command::new(&self.command);
        command
            .args(&self.leading_args)
            .args([
                "-p",
                "Reply with the single word: ok",
                "--model",
                "haiku",
                "--no-session-persistence",
            ])
            .current_dir(&self.work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        for name in OTHER_CREDENTIALS {
            command.env_remove(name);
        }
        if let Some(dir) = &self.config_dir {
            command.env("CLAUDE_CONFIG_DIR", dir);
        }
        let mut child = command
            .spawn()
            .map_err(|error| RunFailure::NotStarted(error.kind().to_string()))?;
        match tokio::time::timeout(self.limit, child.wait()).await {
            Err(_) => Err(RunFailure::TimedOut),
            Ok(Err(error)) => Err(RunFailure::NotStarted(error.kind().to_string())),
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(status)) => Err(RunFailure::Exited(status.code())),
        }
    }
}

/// Renews one login by running `claude`, within the limits above.
pub struct Renewer {
    runner: Arc<dyn RunClaude>,
    record_path: PathBuf,
    log: Log,
}

impl Renewer {
    pub fn new(runner: Arc<dyn RunClaude>, record_path: PathBuf, log: Log) -> Self {
        Self {
            runner,
            record_path,
            log,
        }
    }

    fn record(&self) -> Record {
        std::fs::read_to_string(&self.record_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, record: &Record) {
        if let Ok(value) = serde_json::to_value(record) {
            let _ = usage_file::write(&self.record_path, &value);
        }
    }

    /// Runs `claude` if the token that expired at `expires_at_ms` has been expired long enough and the
    /// limits allow it. `Ok(true)` when it ran and exited well, so that the caller reads the login again;
    /// `Ok(false)` when it did not run.
    pub async fn renew(&self, expires_at_ms: i64, now: i64) -> Result<bool, Failure> {
        if now * 1000 < expires_at_ms + GRACE_SECONDS * 1000 {
            return Ok(false);
        }
        let record = self.record();
        if record.may_run_at(now).is_err() {
            return Ok(false);
        }
        (self.log)("running claude once to renew an expired login".to_owned());
        let result = self.runner.run().await;
        self.save(&record.after_run(now, result.is_ok()));
        match result {
            Ok(()) => Ok(true),
            Err(why) => Err(Failure::NotRenewed(why.to_string())),
        }
    }
}

/// Where the record of one login's runs is kept: in `renewals/` of the data directory (not beside the
/// usage files, whose names are source ids), named for the login's configuration directory (`default`
/// for the default login).
pub fn record_path(data_dir: &Path, config_dir: Option<&Path>) -> PathBuf {
    use sha2::{Digest, Sha256};
    let name = match config_dir {
        None => "default".to_owned(),
        Some(dir) => {
            let digest = Sha256::digest(dir.as_os_str().as_encoded_bytes());
            digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
        }
    };
    data_dir.join("renewals").join(format!("{name}.json"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const HOUR: i64 = 3600;

    fn record(runs: &[i64], failed_at: Option<i64>) -> Record {
        Record {
            runs: runs.to_vec(),
            failed_at,
        }
    }

    #[test]
    fn a_login_with_no_runs_may_run_and_the_gap_between_runs_is_three_hours() {
        assert_eq!(Record::default().may_run_at(0), Ok(()));
        let one = record(&[1000], None);
        assert_eq!(one.may_run_at(1000 + 3 * HOUR - 1), Err(1));
        assert_eq!(one.may_run_at(1000 + 3 * HOUR), Ok(()));
    }

    #[test]
    fn four_runs_in_a_day_are_the_most_and_the_day_is_counted_from_the_oldest() {
        let four = record(&[0, 3 * HOUR, 6 * HOUR, 9 * HOUR], None);
        assert_eq!(
            four.may_run_at(12 * HOUR),
            Err(12 * HOUR),
            "until 24 hours after the first"
        );
        assert_eq!(four.may_run_at(24 * HOUR - 1), Err(1));
        assert_eq!(four.may_run_at(24 * HOUR), Ok(()));
        let three = record(&[0, 3 * HOUR, 6 * HOUR], None);
        assert_eq!(
            three.may_run_at(9 * HOUR),
            Ok(()),
            "three runs leave room for a fourth"
        );
    }

    #[test]
    fn after_a_failed_run_it_waits_six_hours_and_a_good_run_clears_the_failure() {
        let failed = record(&[0], Some(0));
        assert_eq!(failed.may_run_at(6 * HOUR - 1), Err(1));
        assert_eq!(failed.may_run_at(6 * HOUR), Ok(()));
        let later = failed.after_run(7 * HOUR, true);
        assert_eq!(later.failed_at, None);
        assert_eq!(later.runs, [0, 7 * HOUR]);
        assert_eq!(failed.after_run(7 * HOUR, false).failed_at, Some(7 * HOUR));
    }

    #[test]
    fn runs_older_than_a_day_are_forgotten() {
        let old = record(&[0, 3 * HOUR, 6 * HOUR, 9 * HOUR], None);
        let after = old.after_run(25 * HOUR, true);
        assert_eq!(after.runs, [3 * HOUR, 6 * HOUR, 9 * HOUR, 25 * HOUR]);
        assert_eq!(
            after.may_run_at(26 * HOUR),
            Err(2 * HOUR),
            "four runs are in the last day at 26 hours: the gap since the last one holds it back"
        );
        assert_eq!(
            after.may_run_at(28 * HOUR),
            Ok(()),
            "by 28 hours the run at 3 hours is more than a day old, and only three count"
        );
    }

    struct Runs {
        count: Mutex<u32>,
        result: Mutex<Result<(), RunFailure>>,
    }

    #[async_trait]
    impl RunClaude for Runs {
        async fn run(&self) -> Result<(), RunFailure> {
            *self.count.lock().unwrap() += 1;
            self.result.lock().unwrap().clone()
        }
    }

    fn runs(result: Result<(), RunFailure>) -> Arc<Runs> {
        Arc::new(Runs {
            count: Mutex::new(0),
            result: Mutex::new(result),
        })
    }

    fn renewer(dir: &Path, runner: Arc<Runs>, lines: Arc<Mutex<Vec<String>>>) -> Renewer {
        let log: Log = Arc::new(move |line| lines.lock().unwrap().push(line));
        Renewer::new(runner, record_path(dir, None), log)
    }

    #[tokio::test]
    async fn a_token_expired_for_less_than_ten_minutes_is_left_to_the_sessions_that_use_it() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runs(Ok(()));
        let lines = Arc::new(Mutex::new(vec![]));
        let renewer = renewer(dir.path(), runner.clone(), lines.clone());
        let expired_at_ms = 1_000_000_000_000;
        let now = expired_at_ms / 1000;
        assert_eq!(renewer.renew(expired_at_ms, now + 599).await, Ok(false));
        assert_eq!(*runner.count.lock().unwrap(), 0);
        assert_eq!(renewer.renew(expired_at_ms, now + 600).await, Ok(true));
        assert_eq!(*runner.count.lock().unwrap(), 1);
        assert_eq!(lines.lock().unwrap().len(), 1, "the run is logged");
    }

    #[tokio::test]
    async fn the_limits_hold_across_a_restart_because_the_record_is_in_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runs(Ok(()));
        let lines = Arc::new(Mutex::new(vec![]));
        let expired_at_ms = 1_000_000_000_000;
        let first = expired_at_ms / 1000 + 600;
        assert_eq!(
            renewer(dir.path(), runner.clone(), lines.clone())
                .renew(expired_at_ms, first)
                .await,
            Ok(true)
        );
        // A new Renewer, as after a restart, finds the earlier run.
        let again = renewer(dir.path(), runner.clone(), lines.clone());
        assert_eq!(
            again.renew(expired_at_ms, first + 3 * HOUR - 1).await,
            Ok(false)
        );
        assert_eq!(again.renew(expired_at_ms, first + 3 * HOUR).await, Ok(true));
        assert_eq!(*runner.count.lock().unwrap(), 2);
        let mode = std::fs::metadata(record_path(dir.path(), None)).unwrap();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode.permissions()) & 0o777,
            0o600
        );
    }

    #[tokio::test]
    async fn a_run_that_fails_says_why_without_what_claude_printed_and_pauses_for_six_hours() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runs(Err(RunFailure::Exited(Some(1))));
        let renewer = renewer(dir.path(), runner.clone(), Arc::new(Mutex::new(vec![])));
        let expired_at_ms = 1_000_000_000_000;
        let now = expired_at_ms / 1000 + 600;
        let failure = renewer.renew(expired_at_ms, now).await.unwrap_err();
        let Failure::NotRenewed(text) = &failure else {
            panic!("{failure:?}")
        };
        assert!(
            text.contains("exited with status 1") && text.contains("/login"),
            "{text}"
        );
        assert_eq!(
            renewer.renew(expired_at_ms, now + 6 * HOUR - 1).await,
            Ok(false)
        );
        assert!(renewer.renew(expired_at_ms, now + 6 * HOUR).await.is_err());
        assert_eq!(*runner.count.lock().unwrap(), 2);
    }

    #[test]
    fn each_login_has_its_own_record() {
        let data = Path::new("/data");
        let default = record_path(data, None);
        let a = record_path(data, Some(Path::new("/home/alice/.claude-a")));
        let b = record_path(data, Some(Path::new("/home/alice/.claude-b")));
        assert_eq!(default, Path::new("/data/renewals/default.json"));
        assert!(a != b && a != default);
        assert_eq!(
            a,
            record_path(data, Some(Path::new("/home/alice/.claude-a")))
        );
    }

    /// A `claude` that runs `body` as `sh <script> -p …`, in `dir`; it notes what it was run with.
    fn program(dir: &Path, body: &str) -> ClaudeProgram {
        let script = dir.join("claude.sh");
        std::fs::write(&script, format!("{body}\n")).unwrap();
        let mut program = ClaudeProgram::new(
            "/bin/sh".into(),
            Some("/home/alice/.claude-a".into()),
            dir.join("work"),
        );
        program.leading_args = vec![script];
        program
    }

    #[tokio::test]
    async fn claude_runs_with_the_logins_directory_and_no_other_credential() {
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-api-SECRET");
        std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat-SECRET");
        let dir = tempfile::tempdir().unwrap();
        let seen = dir.path().join("seen");
        let body = format!(
            r#"{{ echo "args: $*"; echo "dir: $CLAUDE_CONFIG_DIR"; echo "cwd: $(pwd)"; env | grep -E 'ANTHROPIC_|CLAUDE_CODE_OAUTH' ; echo done; }} > '{}'
for stream in 0 1 2; do [ /dev/fd/$stream -ef /dev/null ] && echo "fd $stream is /dev/null" >> '{seen}'; done
echo sk-ant-oat01-PRINTED; echo sk-ant-oat01-PRINTED >&2"#,
            seen.display(),
            seen = seen.display()
        );
        program(dir.path(), &body).run().await.unwrap();
        let text = std::fs::read_to_string(&seen).unwrap();
        assert!(
            text.contains(
                "args: -p Reply with the single word: ok --model haiku --no-session-persistence"
            ),
            "{text}"
        );
        assert!(text.contains("dir: /home/alice/.claude-a"), "{text}");
        assert!(
            text.contains(&format!(
                "cwd: {}",
                dir.path().join("work").canonicalize().unwrap().display()
            )),
            "{text}"
        );
        assert!(
            !text.contains("SECRET") && !text.contains("ANTHROPIC_"),
            "no other credential: {text}"
        );
        for stream in 0..3 {
            assert!(
                text.contains(&format!("fd {stream} is /dev/null")),
                "stream {stream} is discarded: {text}"
            );
        }
    }

    #[tokio::test]
    async fn claude_that_fails_stalls_or_is_missing_is_told_apart_and_its_output_is_never_kept() {
        let dir = tempfile::tempdir().unwrap();
        let failed = program(dir.path(), "echo sk-ant-oat01-PRINTED; exit 3")
            .run()
            .await
            .unwrap_err();
        assert_eq!(failed, RunFailure::Exited(Some(3)));
        assert!(!failed.to_string().contains("PRINTED"));
        let pid_file = dir.path().join("pid");
        let body = format!("echo $$ > '{}'; sleep 30", pid_file.display());
        let mut slow = program(dir.path(), &body);
        slow.limit = Duration::from_millis(500);
        assert_eq!(slow.run().await.unwrap_err(), RunFailure::TimedOut);
        let pid = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .to_owned();
        let is_running = || {
            std::process::Command::new("kill")
                .args(["-0", &pid])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        };
        for _ in 0..30 {
            if !is_running() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!is_running(), "a run that went over the limit is stopped");
        let mut missing =
            ClaudeProgram::new("/nonexistent/claude".into(), None, dir.path().join("work"));
        missing.leading_args = vec![];
        let error = missing.run().await.unwrap_err();
        assert!(matches!(&error, RunFailure::NotStarted(_)), "{error:?}");
        assert!(error.to_string().contains("PATH"), "{error}");
    }
}
