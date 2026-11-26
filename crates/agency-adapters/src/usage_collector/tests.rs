use super::claude_login::{Login, LoginStore};
use super::*;
use crate::memory::ManualClock;
use agency_proto::client::FundingKind;
use agency_proto::Id;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use serde_json::json;
use std::sync::Mutex;

const NOW: &str = "2026-11-24T12:00:00Z";
const NOW_MS: i64 = 1_795_521_600_000;
const SECRET: &str = "sk-ant-oat01-SECRET";

fn login_text(token: &str, expires_at_ms: i64) -> String {
    json!({"claudeAiOauth": {"accessToken": token, "refreshToken": "r", "expiresAt": expires_at_ms}}).to_string()
}

struct Logins(Vec<String>);

#[async_trait]
impl LoginStore for Logins {
    async fn logins(&self) -> Vec<Login> {
        self.0
            .iter()
            .filter_map(|text| Login::parse(text))
            .collect()
    }
    fn describe(&self) -> String {
        "log in somewhere".into()
    }
}

/// A usage endpoint that answers one thing and notes the Authorization of each request.
struct Endpoint {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

async fn endpoint(
    status: u16,
    body: &'static str,
    headers: &'static [(&'static str, &'static str)],
) -> Endpoint {
    let seen = Arc::new(Mutex::new(vec![]));
    let noted = seen.clone();
    let app = axum::Router::new().route(
        "/usage",
        get(move |request: HeaderMap| {
            let noted = noted.clone();
            async move {
                let authorization = request
                    .get("authorization")
                    .map(|v| v.to_str().unwrap().to_owned());
                noted
                    .lock()
                    .unwrap()
                    .push(authorization.unwrap_or_default());
                let mut answered = HeaderMap::new();
                for (name, value) in headers {
                    answered.insert(*name, value.parse().unwrap());
                }
                (StatusCode::from_u16(status).unwrap(), answered, body)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/usage", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Endpoint { url, seen }
}

fn claude(logins: Vec<String>, url: &str) -> ClaudeUsage {
    let clock = Arc::new(ManualClock::at(NOW));
    ClaudeUsage::new(Arc::new(Logins(logins)), clock, url.to_owned()).unwrap()
}

const ANSWER: &str = r#"{"five_hour":{"utilization":23.0,"resets_at":"2026-11-24T15:00:00Z","note":"x"},"seven_day":null,"extra_usage":{"credits":9},"email":"alice@example.com"}"#;

#[tokio::test]
async fn asks_with_the_token_and_keeps_only_the_windows() {
    let server = endpoint(200, ANSWER, &[]).await;
    let usage = claude(vec![login_text("tok", NOW_MS + 1000)], &server.url);
    assert_eq!(
        usage.read().await,
        Ok(json!({"five_hour": {"utilization": 23.0, "resets_at": "2026-11-24T15:00:00Z"}}))
    );
    assert_eq!(*server.seen.lock().unwrap(), ["Bearer tok"]);
}

#[tokio::test]
async fn uses_the_login_that_lasts_longest_and_never_one_that_ran_out() {
    let server = endpoint(200, ANSWER, &[]).await;
    let logins = vec![
        login_text("old", NOW_MS + 10),
        login_text("new", NOW_MS + 99),
    ];
    claude(logins, &server.url).read().await.unwrap();
    assert_eq!(*server.seen.lock().unwrap(), ["Bearer new"]);

    let expired = claude(vec![login_text("tok", NOW_MS)], &server.url);
    assert_eq!(expired.read().await, Err(Failure::Expired));
    assert_eq!(
        server.seen.lock().unwrap().len(),
        1,
        "an expired token is not sent"
    );
    assert_eq!(
        claude(vec![], &server.url).read().await,
        Err(Failure::NoLogin("log in somewhere".into()))
    );
}

#[tokio::test]
async fn tells_the_failures_apart() {
    let login = || vec![login_text("tok", NOW_MS + 1000)];
    let failure = |status, body, headers| async move {
        let server = endpoint(status, body, headers).await;
        claude(login(), &server.url).read().await.unwrap_err()
    };
    assert_eq!(failure(401, "", &[]).await, Failure::Rejected);
    assert_eq!(
        failure(429, "", &[("retry-after", "120")]).await,
        Failure::RateLimited(Some(Duration::from_secs(120)))
    );
    assert_eq!(failure(429, "", &[]).await, Failure::RateLimited(None));
    assert_eq!(
        failure(500, "", &[]).await,
        Failure::Unavailable("HTTP 500".into())
    );
    assert_eq!(failure(200, "[]", &[]).await, Failure::Malformed);
    assert_eq!(failure(200, r#"{"x":1}"#, &[]).await, Failure::Malformed);
    assert_eq!(failure(200, "not json", &[]).await, Failure::Malformed);
}

/// A source that answers from a list, and counts the reads.
struct Scripted {
    answers: Mutex<Vec<Result<Value, Failure>>>,
    reads: Mutex<u32>,
}

#[async_trait]
impl UsageSource for Scripted {
    async fn read(&self) -> Result<Value, Failure> {
        *self.reads.lock().unwrap() += 1;
        self.answers.lock().unwrap().remove(0)
    }
}

fn spec() -> UsageCollector {
    UsageCollector::Claude {
        config_dir: Some("/home/alice/.claude-a".into()),
    }
}

fn funding(id: &str, collector: Option<UsageCollector>) -> FundingConfig {
    FundingConfig {
        id: Id::parse(id).unwrap(),
        label: id.into(),
        kind: FundingKind::Subscription,
        usage_file: None,
        collector,
    }
}

fn collector(dir: &Path, source: Arc<dyn UsageSource>) -> (Collector, Arc<Mutex<Vec<String>>>) {
    let lines = Arc::new(Mutex::new(vec![]));
    let sink = lines.clone();
    let connect: Connect = Box::new(move |_| Ok(source.clone()));
    let log: Log = Arc::new(move |line| sink.lock().unwrap().push(line));
    (Collector::new(dir.to_owned(), connect, log), lines)
}

fn answer(percent: u32) -> Result<Value, Failure> {
    Ok(json!({"five_hour": {"utilization": percent, "resets_at": "2026-11-24T15:00:00Z"}}))
}

#[tokio::test]
async fn reads_when_due_backs_off_on_failures_and_keeps_the_last_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("usage/a.json");
    let source = Arc::new(Scripted {
        answers: Mutex::new(vec![
            answer(10),
            Err(Failure::Rejected),
            Err(Failure::Rejected),
            answer(30),
        ]),
        reads: Mutex::new(0),
    });
    let (mut collector, lines) = collector(dir.path(), source.clone());
    let sources = [funding("a", Some(spec()))];
    let start = Instant::now();
    let minutes = |n: u64| start + Duration::from_secs(60 * n);

    collector.read_due(&sources, minutes(0)).await;
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        r#"{"five_hour":{"resets_at":"2026-11-24T15:00:00Z","utilization":10}}"#
    );
    collector.read_due(&sources, minutes(4)).await;
    assert_eq!(*source.reads.lock().unwrap(), 1, "not due yet");

    collector.read_due(&sources, minutes(5)).await;
    assert!(
        std::fs::read_to_string(&file).unwrap().contains("10"),
        "a failed read keeps the file"
    );
    collector.read_due(&sources, minutes(9)).await;
    assert_eq!(*source.reads.lock().unwrap(), 2);
    collector.read_due(&sources, minutes(10)).await;
    collector.read_due(&sources, minutes(19)).await;
    assert_eq!(
        *source.reads.lock().unwrap(),
        3,
        "the second failure waits 10 minutes, not 5"
    );
    collector.read_due(&sources, minutes(20)).await;
    assert!(std::fs::read_to_string(&file).unwrap().contains("30"));
    assert_eq!(
        *lines.lock().unwrap(),
        [
            "usage of \"a\" could not be read: the service refused the login's token; trying again in 5 minutes",
            "usage of \"a\" could not be read: the service refused the login's token; trying again in 10 minutes",
            "usage of \"a\" is read again",
        ]
    );
}

#[tokio::test]
async fn a_node_that_restarts_waits_out_the_interval_since_the_file_was_written() {
    let dir = tempfile::tempdir().unwrap();
    usage_file::write(&dir.path().join("usage/a.json"), &json!({})).unwrap();
    let source = Arc::new(Scripted {
        answers: Mutex::new(vec![answer(1)]),
        reads: Mutex::new(0),
    });
    let (mut collector, _) = collector(dir.path(), source.clone());
    let sources = [funding("a", Some(spec()))];
    let start = Instant::now();
    collector.read_due(&sources, start).await;
    assert_eq!(*source.reads.lock().unwrap(), 0);
    collector.read_due(&sources, start + INTERVAL).await;
    assert_eq!(*source.reads.lock().unwrap(), 1);
}

#[tokio::test]
async fn leaves_sources_without_a_collector_alone() {
    let dir = tempfile::tempdir().unwrap();
    let source = Arc::new(Scripted {
        answers: Mutex::new(vec![]),
        reads: Mutex::new(0),
    });
    let (mut collector, _) = collector(dir.path(), source.clone());
    let mut with_file = funding("b", None);
    with_file.usage_file = Some(dir.path().join("b.json"));
    collector
        .read_due(&[funding("a", None), with_file], Instant::now())
        .await;
    assert_eq!(*source.reads.lock().unwrap(), 0);
}

#[tokio::test]
async fn no_token_reaches_a_log_line_or_the_usage_file() {
    let dir = tempfile::tempdir().unwrap();
    let rejecting = endpoint(401, SECRET, &[]).await;
    let accepting = endpoint(200, ANSWER, &[]).await;
    for (id, server) in [("rejected", &rejecting), ("accepted", &accepting)] {
        let usage = claude(vec![login_text(SECRET, NOW_MS + 1000)], &server.url);
        let (mut collector, lines) = collector(dir.path(), Arc::new(usage));
        collector
            .read_due(&[funding(id, Some(spec()))], Instant::now())
            .await;
        let written = std::fs::read_to_string(dir.path().join(format!("usage/{id}.json")))
            .unwrap_or_default();
        let output = format!("{:?}{written}", lines.lock().unwrap());
        assert!(!output.contains(SECRET), "{id}");
        assert!(
            !output.contains("alice@example.com") && !output.contains("credits"),
            "{id}: only the windows are kept"
        );
    }
}
