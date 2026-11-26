//! A team's delivery numbers (docs/protocol.md, "Evaluation"): how fast its pull requests merge, how long
//! its default branch's CI was red, how many merges were reverts, and what its tokens cost for each
//! merged pull request. The facts come from an adapter (GitHub); this module only works them out, so it
//! needs no network, clock, or time zone of its own.
mod evaluate;
mod facts;
mod kinds;
mod period;
mod red;

pub use evaluate::{evaluate, Input};
pub use facts::{MainRun, MergedPullRequest, Verdict};
pub use kinds::{classify, Kind};

#[cfg(test)]
mod tests;
