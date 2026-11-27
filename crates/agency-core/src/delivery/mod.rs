//! A team's delivery numbers (docs/protocol.md, "Evaluation"): how fast its pull requests merge, how long
//! its default branch's CI was red, how many merges were reverts, and what its tokens cost for each
//! merged pull request. The facts come from an adapter (GitHub); this module only works them out, so it
//! needs no network, clock, or time zone of its own.
mod effort;
mod evaluate;
mod facts;
mod kinds;
mod participants;
mod period;
mod red;
mod team_share;

pub use effort::{effort, EffortInput};
pub use evaluate::{evaluate, Input};
pub use facts::{BoardValue, DeliveryFacts, MainRun, MergedPullRequest, Verdict};
pub use kinds::{board_work_type, classify, Kind};
pub use participants::attribute;
pub(crate) use period::proto_date;
pub use period::Period;
pub use team_share::of_team;

#[cfg(test)]
mod effort_tests;
#[cfg(test)]
mod tests;
