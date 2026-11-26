//! The merged pull requests of a repository, with their first commit and the board moves of the issues
//! they close, in one GraphQL query for each page of 50.
use super::gh::{Gh, GhError};
use agency_core::delivery::MergedPullRequest;
use agency_proto::clock::{self, OffsetDateTime};
use serde::Deserialize;

/// The board status whose first appearance on an issue is when work on it started.
const STARTED_STATUS: &str = "In Progress";
const PAGES_AT_MOST: usize = 20;

const QUERY: &str = "query($owner:String!,$name:String!,$after:String){repository(owner:$owner,name:$name){\
pullRequests(states:MERGED,first:50,after:$after,orderBy:{field:UPDATED_AT,direction:DESC}){\
pageInfo{hasNextPage endCursor}\
nodes{title body createdAt mergedAt updatedAt \
commits(first:1){nodes{commit{authoredDate}}} \
closingIssuesReferences(first:5){nodes{timelineItems(first:50,itemTypes:[PROJECT_V2_ITEM_STATUS_CHANGED_EVENT]){\
nodes{... on ProjectV2ItemStatusChangedEvent{createdAt status}}}}}}}}}";

#[derive(Deserialize)]
struct Response {
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    repository: Option<Repository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Repository {
    pull_requests: Page,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    page_info: PageInfo,
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    title: String,
    body: Option<String>,
    created_at: String,
    merged_at: Option<String>,
    updated_at: String,
    commits: Nodes<CommitNode>,
    closing_issues_references: Nodes<Issue>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
struct CommitNode {
    commit: Commit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Commit {
    authored_date: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Issue {
    timeline_items: Nodes<Event>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    created_at: Option<String>,
    status: Option<String>,
}

fn time(raw: &str) -> Option<OffsetDateTime> {
    clock::parse(raw)
}

/// When work on the issues a pull request closes started: the earliest first move to In Progress.
fn issue_started_at(issues: &[Issue]) -> Option<OffsetDateTime> {
    issues
        .iter()
        .flat_map(|issue| &issue.timeline_items.nodes)
        .filter(|event| event.status.as_deref() == Some(STARTED_STATUS))
        .filter_map(|event| time(event.created_at.as_deref()?))
        .min()
}

/// The pull request, if it was merged and its times can be read.
fn merged_pull_request(node: &Node) -> Option<MergedPullRequest> {
    Some(MergedPullRequest {
        title: node.title.clone(),
        body: node.body.clone().unwrap_or_default(),
        opened_at: time(&node.created_at)?,
        merged_at: time(node.merged_at.as_deref()?)?,
        first_commit_at: node
            .commits
            .nodes
            .first()
            .and_then(|commit| time(&commit.commit.authored_date)),
        has_issue: !node.closing_issues_references.nodes.is_empty(),
        issue_started_at: issue_started_at(&node.closing_issues_references.nodes),
    })
}

/// What one page holds: the pull requests merged since `since`, and where the next page starts, when the
/// pages that follow can still hold such a pull request. A pull request that merged is updated at least
/// when it merged, so once a page's last pull request was updated before `since`, the rest were merged
/// before it.
fn read_page(
    text: &str,
    since: OffsetDateTime,
) -> Result<(Vec<MergedPullRequest>, Option<String>), GhError> {
    let response: Response = serde_json::from_str(text).map_err(|_| GhError::Failed)?;
    let page = response
        .data
        .and_then(|data| data.repository)
        .ok_or(GhError::Failed)?
        .pull_requests;
    let found = page
        .nodes
        .iter()
        .filter_map(merged_pull_request)
        .filter(|pull_request| pull_request.merged_at >= since)
        .collect();
    let older = page
        .nodes
        .last()
        .and_then(|node| time(&node.updated_at))
        .is_some_and(|updated| updated < since);
    let next = (page.page_info.has_next_page && !older)
        .then_some(page.page_info.end_cursor)
        .flatten();
    Ok((found, next))
}

/// The pull requests of `owner/name` merged since `since`.
pub async fn merged_since(
    gh: &dyn Gh,
    owner: &str,
    name: &str,
    since: OffsetDateTime,
) -> Result<Vec<MergedPullRequest>, GhError> {
    let mut all = vec![];
    let mut after: Option<String> = None;
    for _ in 0..PAGES_AT_MOST {
        let mut args: Vec<String> = [
            "api",
            "graphql",
            "-f",
            &format!("query={QUERY}"),
            "-f",
            &format!("owner={owner}"),
            "-f",
            &format!("name={name}"),
        ]
        .map(String::from)
        .to_vec();
        if let Some(cursor) = &after {
            args.extend(["-f".into(), format!("after={cursor}")]);
        }
        let (found, next) = read_page(&gh.run(&args).await?, since)?;
        all.extend(found);
        match next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    fn node(
        title: &str,
        merged: Option<&str>,
        updated: &str,
        issues: serde_json::Value,
    ) -> serde_json::Value {
        json!({
            "title": title, "body": null, "createdAt": "2026-11-22T01:00:00Z", "mergedAt": merged, "updatedAt": updated,
            "commits": {"nodes": [{"commit": {"authoredDate": "2026-11-22T00:00:00Z"}}]},
            "closingIssuesReferences": {"nodes": issues},
        })
    }

    fn page(nodes: Vec<serde_json::Value>, next: Option<&str>) -> String {
        json!({"data": {"repository": {"pullRequests": {
            "pageInfo": {"hasNextPage": next.is_some(), "endCursor": next}, "nodes": nodes}}}})
        .to_string()
    }

    fn issue(events: serde_json::Value) -> serde_json::Value {
        json!({"timelineItems": {"nodes": events}})
    }

    #[test]
    fn reads_a_pull_request_and_the_first_move_of_its_issues_to_in_progress() {
        let events = json!([
            {"createdAt": "2026-11-21T09:00:00Z", "status": "Next"},
            {"createdAt": "2026-11-21T10:00:00Z", "status": "In Progress"},
            {"createdAt": "2026-11-21T12:00:00Z", "status": "Release"},
            {"createdAt": "2026-11-21T13:00:00Z", "status": "In Progress"},
            {}
        ]);
        let other = json!([{"createdAt": "2026-11-20T08:00:00Z", "status": "In Progress"}]);
        let text = page(
            vec![node(
                "feat: a",
                Some("2026-11-22T03:00:00Z"),
                "2026-11-22T03:05:00Z",
                json!([issue(events), issue(other)]),
            )],
            None,
        );
        let (found, next) = read_page(&text, at("2026-11-01T00:00:00Z")).unwrap();
        assert_eq!(next, None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "feat: a");
        assert_eq!(found[0].body, "");
        assert_eq!(found[0].opened_at, at("2026-11-22T01:00:00Z"));
        assert_eq!(found[0].merged_at, at("2026-11-22T03:00:00Z"));
        assert_eq!(found[0].first_commit_at, Some(at("2026-11-22T00:00:00Z")));
        assert!(found[0].has_issue);
        assert_eq!(
            found[0].issue_started_at,
            Some(at("2026-11-20T08:00:00Z")),
            "the earliest of the issues' first moves"
        );
    }

    #[test]
    fn only_a_move_to_in_progress_is_a_start() {
        let events = json!([
            {"createdAt": "2026-11-21T08:00:00Z", "status": "Next"},
            {"createdAt": "2026-11-21T09:00:00Z", "status": "Backlog"},
            {"createdAt": "2026-11-21T11:00:00Z", "status": "In Progress"}
        ]);
        let text = page(
            vec![node(
                "feat: a",
                Some("2026-11-22T03:00:00Z"),
                "2026-11-22T03:05:00Z",
                json!([issue(events)]),
            )],
            None,
        );
        let (found, _) = read_page(&text, at("2026-11-01T00:00:00Z")).unwrap();
        assert_eq!(found[0].issue_started_at, Some(at("2026-11-21T11:00:00Z")));
        let before = json!([{"createdAt": "2026-11-21T08:00:00Z", "status": "Next"}]);
        let text = page(
            vec![node(
                "feat: a",
                Some("2026-11-22T03:00:00Z"),
                "2026-11-22T03:05:00Z",
                json!([issue(before)]),
            )],
            None,
        );
        assert_eq!(
            read_page(&text, at("2026-11-01T00:00:00Z")).unwrap().0[0].issue_started_at,
            None
        );
    }

    #[test]
    fn a_pull_request_without_an_issue_or_a_move_has_no_start() {
        let text = page(
            vec![
                node(
                    "fix: a",
                    Some("2026-11-22T03:00:00Z"),
                    "2026-11-22T03:05:00Z",
                    json!([]),
                ),
                node(
                    "fix: b",
                    Some("2026-11-22T04:00:00Z"),
                    "2026-11-22T04:05:00Z",
                    json!([issue(json!([]))]),
                ),
            ],
            None,
        );
        let (found, _) = read_page(&text, at("2026-11-01T00:00:00Z")).unwrap();
        assert_eq!(
            (found[0].has_issue, found[0].issue_started_at),
            (false, None)
        );
        assert_eq!(
            (found[1].has_issue, found[1].issue_started_at),
            (true, None)
        );
    }

    #[test]
    fn leaves_out_what_merged_before_the_period_and_what_never_merged() {
        let text = page(
            vec![
                node(
                    "feat: new",
                    Some("2026-11-22T03:00:00Z"),
                    "2026-11-22T03:05:00Z",
                    json!([]),
                ),
                node("feat: closed", None, "2026-11-22T02:00:00Z", json!([])),
                node(
                    "feat: old",
                    Some("2026-10-01T03:00:00Z"),
                    "2026-11-22T01:00:00Z",
                    json!([]),
                ),
            ],
            None,
        );
        let (found, _) = read_page(&text, at("2026-11-01T00:00:00Z")).unwrap();
        assert_eq!(
            found.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(),
            ["feat: new"]
        );
    }

    #[test]
    fn goes_on_to_the_next_page_only_while_pull_requests_can_still_be_in_the_period() {
        let since = at("2026-11-10T00:00:00Z");
        let recent = page(
            vec![node(
                "a",
                Some("2026-11-20T00:00:00Z"),
                "2026-11-20T00:00:00Z",
                json!([]),
            )],
            Some("c1"),
        );
        assert_eq!(read_page(&recent, since).unwrap().1.as_deref(), Some("c1"));
        let stale = page(
            vec![node(
                "a",
                Some("2026-11-02T00:00:00Z"),
                "2026-11-05T00:00:00Z",
                json!([]),
            )],
            Some("c2"),
        );
        assert_eq!(
            read_page(&stale, since).unwrap().1,
            None,
            "everything after was updated before the period"
        );
        let last = page(
            vec![node(
                "a",
                Some("2026-11-20T00:00:00Z"),
                "2026-11-20T00:00:00Z",
                json!([]),
            )],
            None,
        );
        assert_eq!(read_page(&last, since).unwrap().1, None);
    }

    #[test]
    fn an_answer_that_is_not_a_page_is_a_failure() {
        let since = at("2026-11-01T00:00:00Z");
        for text in [
            "not json",
            "{}",
            r#"{"data":null,"errors":[{"message":"x"}]}"#,
            r#"{"data":{"repository":null}}"#,
        ] {
            assert_eq!(
                read_page(text, since).err(),
                Some(GhError::Failed),
                "{text}"
            );
        }
    }
}
