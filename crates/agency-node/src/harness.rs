//! ハーネスとの境目。ノードはハーネスのファイルを直接読み書きせず、CLI (scrum) だけを呼ぶ。
//! 試験では、同じ形を手元のメモリで返す代わりのハーネスを使う。
use crate::config::HostedTeam;
use agency_proto::{EnvelopeKind, Id, Payload};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 当てる決裁 (封筒の中身と、決めた人の組織の中の id)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub kind: EnvelopeKind,
    pub target: String,
    pub by: Id,
    pub payload: Payload,
}

/// 当てられなかった理由。HTTP の状態に写す (無い = 404、形が違う = 400、ハーネスが動かない = 503)
#[derive(Debug, PartialEq, Eq)]
pub enum ApplyError {
    NotFound(String),
    Invalid(String),
    Unavailable(String),
}

#[derive(Default, Debug)]
pub struct MemoryHarness {
    /// 答えを待っている要求 (id、題)
    pub requests: Vec<(String, String)>,
    pub applied: Vec<Decision>,
}

#[derive(Clone)]
pub enum Harness {
    Cli { scrum: PathBuf },
    Memory(Arc<Mutex<MemoryHarness>>),
}

const TIMEOUT: Duration = Duration::from_secs(20);

/// ハーネスの CLI を動かす。input は標準入力に渡す (答えの文は引数に載せない: -- で始まる文を取り違えないように)
async fn run(scrum: &PathBuf, team: &HostedTeam, args: &[String], input: &str) -> Result<(bool, String, String), String> {
    use tokio::io::AsyncWriteExt;
    let mut command = tokio::process::Command::new(scrum);
    command.args(args).env("SCRUM_STATE_DIR", &team.state_dir).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| format!("ハーネスを起動できない: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).await.map_err(|error| format!("ハーネスに渡せない: {error}"))?;
    }
    let output = tokio::time::timeout(TIMEOUT, child.wait_with_output()).await.map_err(|_| "ハーネスが 20 秒で返らなかった".to_owned())?.map_err(|error| format!("ハーネスが返らない: {error}"))?;
    Ok((output.status.success(), String::from_utf8_lossy(&output.stdout).into_owned(), String::from_utf8_lossy(&output.stderr).into_owned()))
}

fn decision_args(decision: &Decision) -> Vec<String> {
    let verb = match decision.kind {
        EnvelopeKind::Answer => "answer",
        EnvelopeKind::Judge => "judge",
    };
    vec!["request".into(), verb.into(), decision.target.clone(), "--by".into(), decision.by.to_string(), "--json".into()]
}

/// ハーネスの答え `{"ok":false,"status":404,"message":"..."}` を理由に写す
fn failure_of(stdout: &str, stderr: &str) -> ApplyError {
    let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Null);
    let message = parsed["message"].as_str().map_or_else(|| stderr.trim().chars().take(300).collect(), str::to_owned);
    match parsed["status"].as_u64() {
        Some(404) => ApplyError::NotFound(message),
        Some(400) => ApplyError::Invalid(message),
        _ => ApplyError::Unavailable(message),
    }
}

impl Harness {
    pub fn memory() -> (Self, Arc<Mutex<MemoryHarness>>) {
        let inner = Arc::new(Mutex::new(MemoryHarness::default()));
        (Self::Memory(inner.clone()), inner)
    }

    /// チームの今 (人、要求、判断待ち、進み)。ハーネスの `scrum state --json` の出力をそのまま返す
    pub async fn state(&self, team: &HostedTeam) -> Result<Value, String> {
        match self {
            Self::Cli { scrum } => {
                let (ok, stdout, stderr) = run(scrum, team, &["state".into(), "--json".into(), "--id".into(), team.team.to_string(), "--org".into(), team.org.to_string()], "").await?;
                if !ok {
                    return Err(format!("ハーネスがチームの今を返さなかった: {}", stderr.trim()));
                }
                serde_json::from_str(&stdout).map_err(|_| "ハーネスの出力が JSON でない".to_owned())
            }
            Self::Memory(inner) => {
                let state = inner.lock().map_err(|_| "メモリのハーネスが壊れた")?;
                let waiting: Vec<Value> = state.requests.iter().map(|(id, title)| json!({ "id": id, "title": title })).collect();
                Ok(json!({ "id": team.team, "org": team.org, "inbox": { "waitingOwner": waiting } }))
            }
        }
    }

    pub async fn apply(&self, team: &HostedTeam, decision: &Decision) -> Result<(), ApplyError> {
        match self {
            Self::Cli { scrum } => {
                let input = serde_json::to_string(&decision.payload).map_err(|error| ApplyError::Invalid(error.to_string()))?;
                let (ok, stdout, stderr) = run(scrum, team, &decision_args(decision), &input).await.map_err(ApplyError::Unavailable)?;
                if ok {
                    Ok(())
                } else {
                    Err(failure_of(&stdout, &stderr))
                }
            }
            Self::Memory(inner) => {
                let mut state = inner.lock().map_err(|_| ApplyError::Unavailable("メモリのハーネスが壊れた".into()))?;
                if !state.requests.iter().any(|(id, _)| id == &decision.target) {
                    return Err(ApplyError::NotFound(format!("要求が見つからない: {}", decision.target)));
                }
                state.applied.push(decision.clone());
                Ok(())
            }
        }
    }
}
