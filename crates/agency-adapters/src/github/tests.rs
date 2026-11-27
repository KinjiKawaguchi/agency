use super::*;
use agency_core::delivery::Verdict;
use agency_proto::clock;
use serde_json::json;
use std::sync::Mutex;

/// A `gh` that answers from canned pages, and notes the arguments it was given.
struct FakeGh {
    calls: Mutex<Vec<Vec<String>>>,
    fail_with: Option<GhError>,
}

impl FakeGh {
    fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
            fail_with: None,
        }
    }
}

fn graphql_page(title: &str, merged: &str) -> String {
    json!({"data": {"repository": {"pullRequests": {
        "pageInfo": {"hasNextPage": false, "endCursor": null},
        "nodes": [{
            "number": 7, "title": title, "body": "", "createdAt": "2026-11-22T01:00:00Z", "mergedAt": merged, "updatedAt": merged,
            "commits": {"nodes": []}, "closingIssuesReferences": {"nodes": []},
        }]}}}})
    .to_string()
}

#[async_trait]
impl Gh for FakeGh {
    async fn run(&self, args: &[String]) -> Result<String, GhError> {
        self.calls.lock().unwrap().push(args.to_vec());
        if let Some(error) = self.fail_with {
            return Err(error);
        }
        let joined = args.join(" ");
        if joined.contains("graphql") {
            let repo = args.iter().find(|arg| arg.starts_with("name=")).unwrap();
            return Ok(graphql_page(
                &format!("feat: in {repo}"),
                "2026-11-23T03:00:00Z",
            ));
        }
        if joined.contains("--jq .default_branch") {
            return Ok("trunk\n".into());
        }
        Ok("abc\tfailure\t2026-11-23T04:00:00Z\nabd\tsuccess\t2026-11-23T05:00:00Z\n".into())
    }
}

/// The repositories of a team on github.com.
fn on_github(repos: &[&str]) -> DeliveryConfig {
    DeliveryConfig {
        repos: repos.iter().map(|repo| repo.to_string()).collect(),
        host: None,
    }
}

/// The repositories of a team on a GitHub host of its own.
fn on_host(host: &str, repos: &[&str]) -> DeliveryConfig {
    DeliveryConfig {
        host: Some(host.to_owned()),
        ..on_github(repos)
    }
}

fn since() -> agency_proto::clock::OffsetDateTime {
    clock::parse("2026-11-10T00:00:00Z").unwrap()
}

#[tokio::test]
async fn collects_the_pull_requests_and_runs_of_every_repository() {
    let gh = Arc::new(FakeGh::new());
    let delivery = GithubDelivery::new(gh.clone());
    let facts = delivery
        .facts(&on_github(&["acme/app", "acme/web"]), since())
        .await
        .unwrap();
    let titles: Vec<&str> = facts
        .pull_requests
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(titles, ["feat: in name=app", "feat: in name=web"]);
    let repos: Vec<&str> = facts
        .pull_requests
        .iter()
        .map(|p| p.repo.as_str())
        .collect();
    assert_eq!(
        repos,
        ["acme/app", "acme/web"],
        "each pull request knows its repository"
    );
    let runs: Vec<(&str, &str, Verdict)> = facts
        .runs
        .iter()
        .map(|r| (r.repo.as_str(), r.commit.as_str(), r.verdict))
        .collect();
    assert_eq!(
        runs,
        [
            ("acme/app", "abc", Verdict::Failed),
            ("acme/app", "abd", Verdict::Passed),
            ("acme/web", "abc", Verdict::Failed),
            ("acme/web", "abd", Verdict::Passed),
        ]
    );
}

#[tokio::test]
async fn asks_github_for_the_default_branch_and_reads_runs_from_a_week_before_the_period() {
    let gh = Arc::new(FakeGh::new());
    GithubDelivery::new(gh.clone())
        .facts(&on_github(&["acme/app"]), since())
        .await
        .unwrap();
    let calls: Vec<String> = gh
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|args| args.join(" "))
        .collect();
    let runs = calls
        .iter()
        .find(|call| call.contains("actions/runs"))
        .unwrap();
    assert!(
        runs.contains("repos/acme/app/actions/runs --method GET"),
        "{runs}"
    );
    assert!(
        runs.contains("-f branch=trunk"),
        "the default branch, not a guess"
    );
    assert!(runs.contains("-f event=push -f status=completed"));
    assert!(
        runs.contains("-f created=>=2026-11-03"),
        "seven days before the period starts: {runs}"
    );
    assert!(runs.contains("--paginate"));
    let graphql = calls.iter().find(|call| call.contains("graphql")).unwrap();
    assert!(graphql.contains("-f owner=acme -f name=app"));
    assert!(graphql.contains("PROJECT_V2_ITEM_STATUS_CHANGED_EVENT"));
    assert!(
        calls.iter().all(|call| call.starts_with("api ")),
        "only reads through gh api"
    );
}

/// The calls `delivery` makes to a `gh` that answers as `FakeGh` does, one string each.
async fn calls_for(source: &DeliveryConfig) -> Vec<String> {
    let gh = Arc::new(FakeGh::new());
    GithubDelivery::new(gh.clone())
        .facts(source, since())
        .await
        .unwrap();
    let calls = gh.calls.lock().unwrap();
    calls.iter().map(|args| args.join(" ")).collect()
}

#[tokio::test]
async fn tells_gh_the_host_of_a_team_that_names_one_in_every_call() {
    let calls = calls_for(&on_host("git.example.test", &["acme/app"])).await;
    assert_eq!(
        calls.len(),
        3,
        "the pull requests, the default branch, the runs: {calls:?}"
    );
    for call in &calls {
        assert!(
            call.contains("--hostname git.example.test"),
            "every call names the host: {call}"
        );
        assert!(
            call.starts_with("api "),
            "only reads through gh api: {call}"
        );
    }
    let graphql = calls.iter().find(|call| call.contains("graphql")).unwrap();
    assert!(graphql.starts_with("api graphql --hostname git.example.test -f query="));
    assert!(graphql.contains("-f owner=acme -f name=app"));
}

#[tokio::test]
async fn asks_exactly_as_before_when_the_team_names_no_host_or_github_com() {
    let plain = calls_for(&on_github(&["acme/app"])).await;
    assert!(
        plain.iter().all(|call| !call.contains("--hostname")),
        "{plain:?}"
    );
    let named = calls_for(&on_host("GitHub.com", &["acme/app"])).await;
    assert_eq!(named, plain, "github.com named is the call without a host");
}

#[tokio::test]
async fn refuses_a_host_that_is_not_a_host_name_before_asking_github() {
    for host in [
        "",
        "-evil",
        "--hostname",
        "git.example.test --jq x",
        "git example",
        "git.example.test/path",
        "git.example.test:8443",
        "git..example.test",
        ".example.test",
        "example.test.",
        "ex_ample.test",
        "git.example.test?x=1",
    ] {
        let gh = Arc::new(FakeGh::new());
        let error = GithubDelivery::new(gh.clone())
            .facts(&on_host(host, &["acme/app"]), since())
            .await
            .unwrap_err();
        assert!(error.0.contains("not a host name"), "{host:?}: {}", error.0);
        assert!(gh.calls.lock().unwrap().is_empty(), "{host:?} reached gh");
    }
    for host in [
        "git.example.test",
        "ghe",
        "a-b.c-d.example.test",
        "10.0.0.7",
    ] {
        assert!(is_host_name(host), "{host}");
    }
}

#[tokio::test]
async fn says_that_it_is_the_teams_own_host_that_could_not_be_read_and_never_names_it() {
    for (error, wanted) in [
        (GhError::NotInstalled, "not installed"),
        (
            GhError::NotLoggedIn,
            "not logged in to the GitHub host the team names",
        ),
        (
            GhError::RateLimited,
            "rate limit of the GitHub host the team names",
        ),
        (
            GhError::TimedOut,
            "the GitHub host the team names did not answer in time",
        ),
        (
            GhError::Failed,
            "the GitHub host the team names did not answer the request",
        ),
    ] {
        let gh = Arc::new(FakeGh {
            calls: Mutex::new(vec![]),
            fail_with: Some(error),
        });
        let reason = GithubDelivery::new(gh)
            .facts(&on_host("git.example.test", &["acme/app"]), since())
            .await
            .unwrap_err()
            .0;
        assert!(reason.contains(wanted), "{reason}");
        assert!(
            !reason.contains("example.test"),
            "the host is not named: {reason}"
        );
    }
}

#[tokio::test]
async fn names_the_reason_in_plain_words_and_nothing_gh_printed() {
    for (error, wanted) in [
        (GhError::NotInstalled, "not installed"),
        (GhError::NotLoggedIn, "not logged in"),
        (GhError::RateLimited, "rate limit"),
        (GhError::TimedOut, "in time"),
    ] {
        let gh = Arc::new(FakeGh {
            calls: Mutex::new(vec![]),
            fail_with: Some(error),
        });
        let reason = GithubDelivery::new(gh)
            .facts(&on_github(&["acme/app"]), since())
            .await
            .unwrap_err()
            .0;
        assert!(reason.contains(wanted), "{reason}");
    }
}

#[tokio::test]
async fn refuses_a_repository_that_is_not_owner_slash_name_before_asking_github() {
    for repo in [
        "app",
        "acme/",
        "/app",
        "acme/app/extra",
        "acme/a b",
        "acme/app?x=1",
        "acme/app?",
        "acme/app#1",
        "acme/ap p",
        "../app",
        "acme/..",
        "acme/app;rm",
        "",
    ] {
        let gh = Arc::new(FakeGh::new());
        let error = GithubDelivery::new(gh.clone())
            .facts(&on_github(&[repo]), since())
            .await
            .unwrap_err();
        assert!(error.0.contains("owner/name"), "{repo:?}");
        assert!(gh.calls.lock().unwrap().is_empty(), "{repo:?} reached gh");
    }
    assert!(split_repo("my-org/repo.name_1").is_some());
}

/// Run by hand against the real GitHub with the machine's `gh` login:
/// `cargo test -p agency-adapters real_github -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "needs a logged-in gh and the network"]
async fn real_github() {
    let since = clock::parse("2026-11-20T00:00:00Z").unwrap();
    let facts = GithubDelivery::default()
        .facts(&on_github(&["KinjiKawaguchi/agency"]), since)
        .await
        .unwrap();
    println!(
        "{} merged pull requests, {} runs",
        facts.pull_requests.len(),
        facts.runs.len()
    );
    assert!(!facts.pull_requests.is_empty() && !facts.runs.is_empty());
    let input = agency_core::delivery::Input {
        now: clock::OffsetDateTime::now_utc(),
        offset: clock::UtcOffset::from_hms(9, 0, 0).unwrap(),
        days: 7,
        pull_requests: &facts.pull_requests,
        runs: &facts.runs,
        cost_by_day: &[],
    };
    let team = agency_core::delivery::evaluate(agency_proto::Id::parse("app").unwrap(), &input);
    println!("{} to {}", team.from.as_str(), team.until.as_str());
    println!("{:?}", team.summary);
    for pull_request in &facts.pull_requests {
        if agency_core::delivery::classify(&pull_request.title, &pull_request.body)
            == agency_core::delivery::Kind::Revert
        {
            println!("REVERT: {}", pull_request.title);
        }
    }
}
