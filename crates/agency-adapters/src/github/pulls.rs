//! The merged pull requests of a repository, with their first commit and the board moves of the issues
//! they close, in one GraphQL query for each page of 50.
use super::gh::{Gh, GhError};
use agency_core::delivery::{board_work_type, MergedPullRequest};
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::view::WorkType;
use serde::Deserialize;

/// The board status whose first appearance on an issue is when work on it started.
const STARTED_STATUS: &str = "In Progress";
/// The label of an issue whose fix is incident work.
const INCIDENT_LABEL: &str = "incident";
const PAGES_AT_MOST: usize = 20;

const QUERY: &str = "query($owner:String!,$name:String!,$after:String){repository(owner:$owner,name:$name){\
pullRequests(states:MERGED,first:50,after:$after,orderBy:{field:UPDATED_AT,direction:DESC}){\
pageInfo{hasNextPage endCursor}\
nodes{number title body createdAt mergedAt updatedAt \
commits(first:1){nodes{commit{authoredDate}}} \
closingIssuesReferences(first:5){nodes{labels(first:20){nodes{name}} \
projectItems(first:10){nodes{fieldValueByName(name:\"Work type\"){... on ProjectV2ItemFieldSingleSelectValue{name}}}} \
timelineItems(first:50,itemTypes:[PROJECT_V2_ITEM_STATUS_CHANGED_EVENT]){\
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
    number: u64,
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
    labels: Nodes<Label>,
    project_items: Nodes<ProjectItem>,
    timeline_items: Nodes<Event>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItem {
    /// `null` when the project has no such field or the item has no value; `{}` for a value of another kind.
    field_value_by_name: Option<FieldValue>,
}

#[derive(Deserialize)]
struct FieldValue {
    name: Option<String>,
}

#[derive(Deserialize)]
struct Label {
    name: String,
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

/// Whether an issue a pull request closes carries the label that says the work was an incident.
fn closes_incident(issues: &[Issue]) -> bool {
    issues
        .iter()
        .flat_map(|issue| &issue.labels.nodes)
        .any(|label| label.name == INCIDENT_LABEL)
}

/// The board's "Work type" of the first issue a pull request closes that has one, in the order GitHub lists
/// the issues and their projects. An option that names none of the five work types is no value.
fn work_type(issues: &[Issue]) -> Option<WorkType> {
    issues
        .iter()
        .flat_map(|issue| &issue.project_items.nodes)
        .filter_map(|item| item.field_value_by_name.as_ref()?.name.as_deref())
        .find_map(board_work_type)
}

/// The pull request, if it was merged and its times can be read.
fn merged_pull_request(repo: &str, node: &Node) -> Option<MergedPullRequest> {
    Some(MergedPullRequest {
        repo: repo.to_owned(),
        number: node.number,
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
        closes_incident: closes_incident(&node.closing_issues_references.nodes),
        work_type: work_type(&node.closing_issues_references.nodes),
    })
}

/// What one page holds: the pull requests merged since `since`, and where the next page starts, when the
/// pages that follow can still hold such a pull request. A pull request that merged is updated at least
/// when it merged, so once a page's last pull request was updated before `since`, the rest were merged
/// before it.
fn read_page(
    repo: &str,
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
        .filter_map(|node| merged_pull_request(repo, node))
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
        let (found, next) = read_page(&format!("{owner}/{name}"), &gh.run(&args).await?, since)?;
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
            "number": 7, "title": title, "body": null, "createdAt": "2026-11-22T01:00:00Z", "mergedAt": merged, "updatedAt": updated,
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
        labeled_issue(&[], events)
    }

    fn labeled_issue(labels: &[&str], events: serde_json::Value) -> serde_json::Value {
        let labels: Vec<_> = labels.iter().map(|name| json!({"name": name})).collect();
        json!({"labels": {"nodes": labels}, "projectItems": {"nodes": []}, "timelineItems": {"nodes": events}})
    }

    /// An issue on projects whose "Work type" values are `values` (`None` for a project without the field,
    /// which GitHub answers with `null`, and `Some("")` for a value of another kind, answered as `{}`).
    fn issue_on_boards(values: &[Option<&str>]) -> serde_json::Value {
        let items: Vec<_> = values
            .iter()
            .map(|value| match value {
                None => json!({"fieldValueByName": null}),
                Some("") => json!({"fieldValueByName": {}}),
                Some(name) => json!({"fieldValueByName": {"name": name}}),
            })
            .collect();
        json!({"labels": {"nodes": []}, "projectItems": {"nodes": items}, "timelineItems": {"nodes": []}})
    }

    #[test]
    fn a_pull_request_carries_its_repository_and_number_so_that_a_runtime_can_name_its_author() {
        let mut numbered = node(
            "feat: x",
            Some("2026-11-22T02:00:00Z"),
            "2026-11-22T02:00:00Z",
            json!([]),
        );
        numbered["number"] = json!(1234);
        let text = page(vec![numbered], None);
        let (found, _) = read_page("acme/web", &text, at("2026-11-01T00:00:00Z")).unwrap();
        assert_eq!(
            (found[0].repo.as_str(), found[0].number),
            ("acme/web", 1234)
        );
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
        let (found, next) = read_page("acme/app", &text, at("2026-11-01T00:00:00Z")).unwrap();
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
        let (found, _) = read_page("acme/app", &text, at("2026-11-01T00:00:00Z")).unwrap();
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
            read_page("acme/app", &text, at("2026-11-01T00:00:00Z"))
                .unwrap()
                .0[0]
                .issue_started_at,
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
        let (found, _) = read_page("acme/app", &text, at("2026-11-01T00:00:00Z")).unwrap();
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
        let (found, _) = read_page("acme/app", &text, at("2026-11-01T00:00:00Z")).unwrap();
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
        assert_eq!(
            read_page("acme/app", &recent, since).unwrap().1.as_deref(),
            Some("c1")
        );
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
            read_page("acme/app", &stale, since).unwrap().1,
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
        assert_eq!(read_page("acme/app", &last, since).unwrap().1, None);
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
                read_page("acme/app", text, since).err(),
                Some(GhError::Failed),
                "{text}"
            );
        }
    }

    #[test]
    fn a_pull_request_that_closes_an_issue_labeled_incident_says_so() {
        let merged = Some("2026-11-22T02:00:00Z");
        let nodes = vec![
            node(
                "fix: a",
                merged,
                "2026-11-22T02:00:00Z",
                json!([labeled_issue(&["bug", "incident"], json!([]))]),
            ),
            node(
                "fix: b",
                merged,
                "2026-11-22T02:00:00Z",
                json!([labeled_issue(&["bug", "incident-review"], json!([]))]),
            ),
            node("fix: c", merged, "2026-11-22T02:00:00Z", json!([])),
        ];
        let (found, _) =
            read_page("acme/web", &page(nodes, None), at("2026-11-01T00:00:00Z")).unwrap();
        let flags: Vec<bool> = found.iter().map(|one| one.closes_incident).collect();
        assert_eq!(flags, [true, false, false]);
    }

    fn work_types(issues: Vec<serde_json::Value>) -> Vec<Option<WorkType>> {
        let merged = Some("2026-11-22T02:00:00Z");
        let nodes = vec![node(
            "feat: a",
            merged,
            "2026-11-22T02:00:00Z",
            json!(issues),
        )];
        let (found, _) =
            read_page("acme/web", &page(nodes, None), at("2026-11-01T00:00:00Z")).unwrap();
        found.iter().map(|one| one.work_type).collect()
    }

    #[test]
    fn the_boards_work_type_of_the_issue_a_pull_request_closes_is_read() {
        assert_eq!(
            work_types(vec![issue_on_boards(&[Some("Tech debt")])]),
            [Some(WorkType::Debt)]
        );
    }

    #[test]
    fn a_project_without_the_field_or_with_another_option_gives_no_work_type() {
        assert_eq!(
            work_types(vec![issue_on_boards(&[None, Some(""), Some("Spike")])]),
            [None]
        );
        assert_eq!(work_types(vec![]), [None]);
    }

    #[test]
    fn the_first_issue_and_project_that_has_a_work_type_decides() {
        let issues = vec![
            issue_on_boards(&[None]),
            issue_on_boards(&[Some("Spike"), Some("improvement"), Some("incident")]),
            issue_on_boards(&[Some("maintenance")]),
        ];
        assert_eq!(work_types(issues), [Some(WorkType::Improvement)]);
    }
}
