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
        renew: true,
        command: None,
    }
}

fn funding(id: &str, collector: Option<UsageCollector>) -> FundingConfig {
    FundingConfig {
        id: Id::parse(id).unwrap(),
        label: id.into(),
        kind: FundingKind::Subscription,
        usage_file: None,
        collector,
        seven_day_threshold: None,
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

/// A login store whose logins a run of `claude` can change, as a real run changes the stored login.
struct SharedLogins(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl LoginStore for SharedLogins {
    async fn logins(&self) -> Vec<Login> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|text| Login::parse(text))
            .collect()
    }
    fn describe(&self) -> String {
        "log in somewhere".into()
    }
}

/// A `claude` that, when run, stores `renewed` as the login (when it has one) and exits with `result`.
struct FakeClaude {
    logins: Arc<Mutex<Vec<String>>>,
    renewed: Option<String>,
    result: Result<(), renew::RunFailure>,
    runs: Mutex<u32>,
}

#[async_trait]
impl renew::RunClaude for FakeClaude {
    async fn run(&self) -> Result<(), renew::RunFailure> {
        *self.runs.lock().unwrap() += 1;
        if let Some(renewed) = &self.renewed {
            *self.logins.lock().unwrap() = vec![renewed.clone()];
        }
        self.result.clone()
    }
}

struct Renewing {
    usage: ClaudeUsage,
    claude: Arc<FakeClaude>,
    _dir: tempfile::TempDir,
}

impl Renewing {
    fn runs(&self) -> u32 {
        *self.claude.runs.lock().unwrap()
    }
}

/// A login that expired `expired_ago_minutes` ago, and a `claude` that renews it (`renews`) or not.
fn renewing(
    expired_ago_minutes: i64,
    renews: bool,
    result: Result<(), renew::RunFailure>,
    url: &str,
) -> Renewing {
    let expired = login_text(
        "sk-ant-oat01-EXPIRED",
        NOW_MS - expired_ago_minutes * 60_000,
    );
    let renewed = login_text("sk-ant-oat01-RENEWED", NOW_MS + 8 * 3600 * 1000);
    let logins = Arc::new(Mutex::new(vec![expired]));
    let claude = Arc::new(FakeClaude {
        logins: logins.clone(),
        renewed: renews.then_some(renewed),
        result,
        runs: Mutex::new(0),
    });
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::at(NOW));
    let log: Log = Arc::new(|_| {});
    let renewer = renew::Renewer::new(claude.clone(), renew::record_path(dir.path(), None), log);
    let usage = ClaudeUsage::new(Arc::new(SharedLogins(logins)), clock, url.to_owned())
        .unwrap()
        .with_renewer(renewer);
    Renewing {
        usage,
        claude,
        _dir: dir,
    }
}

#[tokio::test]
async fn an_expired_login_is_renewed_by_claude_and_read_again_with_the_new_token() {
    let endpoint = endpoint(200, ANSWER, &[]).await;
    let case = renewing(30, true, Ok(()), &endpoint.url);
    let usage = case.usage.read().await.unwrap();
    assert!(usage["five_hour"].is_object());
    assert_eq!(case.runs(), 1);
    assert_eq!(
        *endpoint.seen.lock().unwrap(),
        ["Bearer sk-ant-oat01-RENEWED"],
        "the expired token is never sent"
    );
    case.usage.read().await.unwrap();
    assert_eq!(case.runs(), 1, "a login that is fresh again is not renewed");
}

#[tokio::test]
async fn a_token_that_expired_a_moment_ago_is_left_to_the_sessions_that_use_it() {
    let endpoint = endpoint(200, ANSWER, &[]).await;
    let case = renewing(5, true, Ok(()), &endpoint.url);
    assert_eq!(case.usage.read().await.unwrap_err(), Failure::Expired);
    assert_eq!(case.runs(), 0);
    assert!(endpoint.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_renewal_that_fails_says_so_and_one_that_changes_nothing_is_not_repeated_within_three_hours(
) {
    let endpoint = endpoint(200, ANSWER, &[]).await;
    let failing = renewing(
        30,
        false,
        Err(renew::RunFailure::Exited(Some(1))),
        &endpoint.url,
    );
    match failing.usage.read().await.unwrap_err() {
        Failure::NotRenewed(why) => assert!(why.contains("exited with status 1"), "{why}"),
        other => panic!("{other:?}"),
    }
    let unchanged = renewing(30, false, Ok(()), &endpoint.url);
    assert_eq!(unchanged.usage.read().await.unwrap_err(), Failure::Expired);
    assert_eq!(unchanged.usage.read().await.unwrap_err(), Failure::Expired);
    assert_eq!(
        unchanged.runs(),
        1,
        "the second read within the gap runs nothing"
    );
    assert!(endpoint.seen.lock().unwrap().is_empty());
}

#[test]
fn a_source_with_renew_off_has_no_renewer() {
    let log: Log = Arc::new(|_| {});
    let dir = Path::new("/data");
    let login = Some(Path::new("/home/alice/.claude-a"));
    assert!(renewer(false, None, login, dir, &log).is_none());
    assert!(renewer(true, None, login, dir, &log).is_some());
    assert!(renewer(true, Some(Path::new("/opt/bin/claude")), None, dir, &log).is_some());
}

#[tokio::test]
async fn a_login_that_is_fresh_or_missing_is_never_renewed() {
    let endpoint = endpoint(200, ANSWER, &[]).await;
    let fresh = renewing(-60, true, Ok(()), &endpoint.url);
    fresh.usage.read().await.unwrap();
    assert_eq!(fresh.runs(), 0);
    let missing = renewing(30, true, Ok(()), &endpoint.url);
    missing.claude.logins.lock().unwrap().clear();
    assert!(matches!(
        missing.usage.read().await.unwrap_err(),
        Failure::NoLogin(_)
    ));
    assert_eq!(missing.runs(), 0);
}
