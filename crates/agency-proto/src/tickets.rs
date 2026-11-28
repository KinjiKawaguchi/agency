//! A team's tickets (`tickets` in agency-runtime/1): the epics the team answers for with every child, open
//! and closed, and its tickets that belong to no epic, with what the board says of each (docs/protocol.md,
//! "Tickets"). The runtime reads the board; this module only says what an answer holds and checks it.
//! An optional field is left out when absent, never written as `null`.
use crate::envelope::is_target;
use crate::ids::Id;
use crate::org::printable;
use crate::participant_metrics::valid_repo;
use crate::timestamp::Timestamp;
use crate::view::{limits, WorkStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// How many epics one answer may hold, as a view's `epics`.
pub const MAX_EPICS: usize = limits::EPICS;
/// How many tickets one answer may hold in all (epics, their children, and the tickets in no epic), as a
/// view's `work`.
pub const MAX_TICKETS: usize = limits::WORK;
/// How many open pull requests one ticket may list.
pub const MAX_PULL_REQUESTS: usize = 50;

/// What a team's runtime says of its tickets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tickets {
    pub org: Id,
    pub team: Id,
    pub generated_at: Timestamp,
    /// The epics whose board Team is this team, each with every child, whatever the child's own Team.
    pub epics: Vec<Epic>,
    /// The team's tickets that belong to no epic.
    pub without_epic: Vec<Ticket>,
}

/// An epic: a ticket of its own, and its children, open and closed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Epic {
    #[serde(flatten)]
    #[ts(flatten)]
    pub ticket: Ticket,
    pub children: Vec<Ticket>,
}

/// One ticket, as the board has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct Ticket {
    /// The ticket's item reference (see "TeamView" in docs/protocol.md). Nothing parses it.
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub reference: String,
    pub title: String,
    /// Its board status; a closed ticket is `done`.
    pub status: WorkStatus,
    /// The board's Team option, as text. It is the board's own value, not an org team id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    /// Who holds it: a name of the team's runtime, as a work item's `assignee`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<u32>,
    /// The ticket's own priority on the board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// What its work is ordered by: its own priority, else its epic's. The runtime works it out, since its
    /// dispatch orders work by the same rule; a node passes it on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_priority: Option<Priority>,
    /// The target a priority decision names, under an envelope target's rule. Given only when the runtime
    /// can take a priority for the ticket; a client offers one only then and never derives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prioritize_with: Option<String>,
    /// Its open pull requests.
    pub pull_requests: Vec<OpenPullRequest>,
}

/// The order in which work is handed out (docs/glossary.md, "Priority"). `deferred` is not handed out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Priority {
    Highest,
    High,
    Medium,
    Low,
    Deferred,
}

/// An open pull request of a ticket, named as GitHub names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenPullRequest {
    /// `owner/name` on GitHub.
    pub repo: String,
    #[ts(type = "number")]
    pub number: u64,
    pub title: String,
}

impl Tickets {
    /// Checks that this is a well-formed answer of the team that was asked, within the limits, with each
    /// ticket listed once.
    pub fn check(&self, org: &Id, team: &Id) -> Result<(), String> {
        if &self.org != org || &self.team != team {
            return Err(format!(
                "reported the tickets of {}/{} instead of {org}/{team}",
                self.org, self.team
            ));
        }
        if self.epics.len() > MAX_EPICS {
            return Err(format!("reported more than {MAX_EPICS} epics"));
        }
        let all = self.all();
        if all.len() > MAX_TICKETS {
            return Err(format!("reported more than {MAX_TICKETS} tickets"));
        }
        let mut seen = HashSet::new();
        for ticket in all {
            ticket.check()?;
            if !seen.insert(ticket.reference.as_str()) {
                return Err(format!("reported {} twice", ticket.reference));
            }
        }
        Ok(())
    }

    /// The answer with every text the board's people wrote (titles, Team options, assignees, pull request
    /// titles) made printable: each character that hides or reorders how a line reads becomes `?`. A node
    /// serves an answer only this way, so no client has to.
    pub fn printable(self) -> Self {
        Self {
            epics: self
                .epics
                .into_iter()
                .map(|epic| Epic {
                    ticket: epic.ticket.printable(),
                    children: epic.children.into_iter().map(Ticket::printable).collect(),
                })
                .collect(),
            without_epic: self
                .without_epic
                .into_iter()
                .map(Ticket::printable)
                .collect(),
            ..self
        }
    }

    /// Every ticket of the answer: each epic, then its children, then the tickets in no epic.
    pub fn all(&self) -> Vec<&Ticket> {
        self.epics
            .iter()
            .flat_map(|epic| std::iter::once(&epic.ticket).chain(&epic.children))
            .chain(&self.without_epic)
            .collect()
    }
}

impl Ticket {
    fn printable(self) -> Self {
        Self {
            title: printable(&self.title),
            team: self.team.as_deref().map(printable),
            assignee: self.assignee.as_deref().map(printable),
            pull_requests: self
                .pull_requests
                .into_iter()
                .map(|pull_request| OpenPullRequest {
                    title: printable(&pull_request.title),
                    ..pull_request
                })
                .collect(),
            ..self
        }
    }

    fn check(&self) -> Result<(), String> {
        let reference = &self.reference;
        if reference.is_empty() || reference.chars().count() > limits::NAME {
            return Err("reported a ticket without a usable ref".into());
        }
        if self.title.chars().count() > limits::TITLE {
            return Err(format!("the title of {reference} is too long"));
        }
        let mut names = [&self.team, &self.assignee].into_iter().flatten();
        if names.any(|name| name.chars().count() > limits::NAME) {
            return Err(format!("a name of {reference} is too long"));
        }
        if self
            .prioritize_with
            .as_deref()
            .is_some_and(|target| !is_target(target))
        {
            return Err(format!(
                "{reference} names a priority target that is malformed"
            ));
        }
        self.check_pull_requests()
    }

    fn check_pull_requests(&self) -> Result<(), String> {
        let reference = &self.reference;
        if self.pull_requests.len() > MAX_PULL_REQUESTS {
            return Err(format!(
                "{reference} lists more than {MAX_PULL_REQUESTS} pull requests"
            ));
        }
        let mut seen = HashSet::new();
        for OpenPullRequest {
            repo,
            number,
            title,
        } in &self.pull_requests
        {
            if !valid_repo(repo) || *number == 0 {
                return Err(format!(
                    "{reference} lists a pull request that is not owner/name and a number"
                ));
            }
            if title.chars().count() > limits::TITLE {
                return Err(format!("the title of {repo}#{number} is too long"));
            }
            if !seen.insert((repo.as_str(), *number)) {
                return Err(format!("{reference} lists {repo}#{number} twice"));
            }
        }
        Ok(())
    }
}
