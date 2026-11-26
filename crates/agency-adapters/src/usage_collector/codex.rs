//! A ChatGPT plan's usage windows, asked of `codex app-server` (JSON-RPC on its standard input and
//! output, method `account/rateLimits/read`). Codex holds the login and sends its own requests, so
//! this reads no token. It starts one app server for each read and ends it afterwards.
use super::schedule::Failure;
use super::UsageSource;
use agency_proto::clock::{self, OffsetDateTime};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// How long a read waits for the answer.
pub const ANSWER_WITHIN: Duration = Duration::from_secs(30);
const REQUEST_ID: u64 = 2;

pub struct CodexUsage {
    command: PathBuf,
    codex_home: Option<PathBuf>,
    answer_within: Duration,
}

impl CodexUsage {
    pub fn new(command: PathBuf, codex_home: Option<PathBuf>, answer_within: Duration) -> Self {
        Self {
            command,
            codex_home,
            answer_within,
        }
    }

    fn app_server(&self) -> Command {
        let mut command = Command::new(&self.command);
        command.arg("app-server");
        if let Some(home) = &self.codex_home {
            command.env("CODEX_HOME", home);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        command
    }

    async fn ask(&self) -> Result<Value, Failure> {
        let mut child = self.app_server().spawn().map_err(|error| {
            Failure::Unavailable(format!("cannot start codex: {}", error.kind()))
        })?;
        let mut stdin = child.stdin.take().ok_or(Failure::Malformed)?;
        let stdout = child.stdout.take().ok_or(Failure::Malformed)?;
        let requests = [
            json!({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "agency-usage", "title": null, "version": env!("CARGO_PKG_VERSION")}}}),
            json!({"method": "initialized"}),
            json!({"id": REQUEST_ID, "method": "account/rateLimits/read"}),
        ];
        for request in requests {
            let line = format!("{request}\n");
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(unavailable)?;
        }
        let answer = answer(BufReader::new(stdout)).await;
        drop(stdin);
        let _ = child.kill().await;
        answer
    }
}

fn unavailable(error: std::io::Error) -> Failure {
    Failure::Unavailable(format!("codex: {}", error.kind()))
}

/// The response to the rate limits request: the app server also sends notifications of its own and
/// answers the other requests, which are skipped.
async fn answer(stdout: BufReader<tokio::process::ChildStdout>) -> Result<Value, Failure> {
    let mut lines = stdout.lines();
    while let Some(line) = lines.next_line().await.map_err(unavailable)? {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message["id"].as_u64() == Some(REQUEST_ID) {
            return Ok(message);
        }
    }
    Err(Failure::Unavailable("codex ended without an answer".into()))
}

/// A window of the plan as the usage file holds it, by how long it is: 5 hours or 7 days.
fn window(raw: &Value) -> Option<(&'static str, Value)> {
    let name = match raw["windowDurationMins"].as_u64()? {
        300 => "five_hour",
        10_080 => "seven_day",
        _ => return None,
    };
    let used = raw["usedPercent"].as_f64()?;
    let resets_at = OffsetDateTime::from_unix_timestamp(raw["resetsAt"].as_i64()?).ok()?;
    Some((
        name,
        json!({"utilization": used, "resets_at": clock::format(resets_at)}),
    ))
}

fn windows(response: &Value) -> Result<Value, Failure> {
    if let Some(message) = response["error"]["message"].as_str() {
        return Err(if message.contains("authentication required") {
            Failure::NoLogin(
                "run `codex login` with this CODEX_HOME and sign in with ChatGPT".into(),
            )
        } else {
            Failure::Unavailable("codex refused the request".into())
        });
    }
    let limits = &response["result"]["rateLimits"];
    if !limits.is_object() {
        return Err(Failure::Malformed);
    }
    let found: Map<String, Value> = ["primary", "secondary"]
        .iter()
        .filter_map(|position| window(&limits[*position]))
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    if found.is_empty() {
        return Err(Failure::NoWindows);
    }
    Ok(Value::Object(found))
}

#[async_trait]
impl UsageSource for CodexUsage {
    async fn read(&self) -> Result<Value, Failure> {
        let response = tokio::time::timeout(self.answer_within, self.ask())
            .await
            .map_err(|_| Failure::Unavailable("codex did not answer in time".into()))??;
        windows(&response)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A `codex` that runs `body` (a shell script) instead of an app server.
    fn fake_codex(dir: &std::path::Path, body: &str) -> CodexUsage {
        let path = dir.join("codex");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        CodexUsage::new(path, Some(dir.join("home")), Duration::from_secs(5))
    }

    /// Answers the initialize request, sends a notification of its own, then answers the rate limits
    /// request with `answer`. It checks that it is asked for an app server and for the right home.
    fn app_server(answer: &str) -> String {
        format!(
            r#"[ "$1" = app-server ] || exit 9
case "$CODEX_HOME" in */home) ;; *) exit 8 ;; esac
read init
echo '{{"id":1,"result":{{"userAgent":"x"}}}}'
echo '{{"method":"remoteControl/status/changed","params":{{"status":"disabled"}}}}'
read initialized
read limits
echo '{answer}'
sleep 30"#
        )
    }

    const PLAN: &str = r#"{"id":2,"result":{"rateLimits":{"limitId":"codex","planType":"plus","primary":{"usedPercent":12,"windowDurationMins":300,"resetsAt":1795532400},"secondary":{"usedPercent":57,"windowDurationMins":10080,"resetsAt":1796000000},"credits":{"balance":"9"}}}}"#;

    #[tokio::test]
    async fn reads_both_windows_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let usage = fake_codex(dir.path(), &app_server(PLAN));
        assert_eq!(
            usage.read().await,
            Ok(json!({
                "five_hour": {"utilization": 12.0, "resets_at": "2026-11-24T15:00:00Z"},
                "seven_day": {"utilization": 57.0, "resets_at": "2026-11-30T00:53:20Z"},
            }))
        );
    }

    #[tokio::test]
    async fn names_a_window_by_its_length_and_not_by_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let weekly_only = r#"{"id":2,"result":{"rateLimits":{"primary":{"usedPercent":3,"windowDurationMins":10080,"resetsAt":1796000000},"secondary":null}}}"#;
        let usage = fake_codex(dir.path(), &app_server(weekly_only));
        let read = usage.read().await.unwrap();
        assert_eq!(
            read.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["seven_day"]
        );
    }

    #[tokio::test]
    async fn tells_the_failures_apart() {
        let dir = tempfile::tempdir().unwrap();
        let failure = |body: String| {
            let usage = fake_codex(dir.path(), &body);
            async move { usage.read().await.unwrap_err() }
        };
        let signed_out = r#"{"error":{"code":-32600,"message":"codex account authentication required to read rate limits"},"id":2}"#;
        assert!(matches!(
            failure(app_server(signed_out)).await,
            Failure::NoLogin(_)
        ));
        let no_windows = r#"{"id":2,"result":{"rateLimits":{"primary":null,"secondary":null}}}"#;
        assert_eq!(failure(app_server(no_windows)).await, Failure::NoWindows);
        let other = r#"{"id":2,"result":{"something":1}}"#;
        assert_eq!(failure(app_server(other)).await, Failure::Malformed);
        assert_eq!(
            failure("exit 0".into()).await,
            Failure::Unavailable("codex ended without an answer".into())
        );
    }

    #[tokio::test]
    async fn gives_up_on_an_app_server_that_does_not_answer() {
        let dir = tempfile::tempdir().unwrap();
        let mut usage = fake_codex(dir.path(), "sleep 30");
        usage.answer_within = Duration::from_millis(300);
        assert_eq!(
            usage.read().await,
            Err(Failure::Unavailable("codex did not answer in time".into()))
        );
        usage.command = dir.path().join("missing");
        assert_eq!(
            usage.read().await,
            Err(Failure::Unavailable(
                "cannot start codex: entity not found".into()
            ))
        );
    }
}
