//! What the delivery numbers are worked out from: facts about the pull requests merged into a team's
//! repositories and about the CI runs on their default branches. An adapter reads them from where they
//! live (GitHub); nothing here reaches outside.
use agency_proto::clock::OffsetDateTime;

/// What a team's delivery numbers are worked out from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeliveryFacts {
    pub pull_requests: Vec<MergedPullRequest>,
    pub runs: Vec<MainRun>,
}

/// A pull request that was merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedPullRequest {
    /// The repository (`owner/name`) and the number the pull request has in it.
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub opened_at: OffsetDateTime,
    pub merged_at: OffsetDateTime,
    /// When its first commit was made.
    pub first_commit_at: Option<OffsetDateTime>,
    /// Whether it closes an issue. Only then does it have an issue lead time.
    pub has_issue: bool,
    /// When the issue first moved to In Progress on the board, if it did.
    pub issue_started_at: Option<OffsetDateTime>,
    /// Whether an issue it closes carries the label `incident`.
    pub closes_incident: bool,
}

/// How one workflow run on the default branch ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It passed (or was skipped, which fails nothing).
    Passed,
    /// It failed or timed out.
    Failed,
    /// It was cancelled, for example by a newer push: it says nothing about the commit.
    Cancelled,
}

/// A workflow run on the default branch of one repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MainRun {
    /// The repository (`owner/name`): each repository's branch is red or green by itself.
    pub repo: String,
    pub commit: String,
    pub finished_at: OffsetDateTime,
    pub verdict: Verdict,
}
