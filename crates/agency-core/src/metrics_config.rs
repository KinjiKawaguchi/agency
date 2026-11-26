//! The settings of the node's measurements: the objectives its operator sets for how fast a group of routes
//! should answer.

use agency_proto::metrics::ROUTE_GROUPS;
use serde::{Deserialize, Serialize};

/// How many objectives a node may have.
pub const MOST_OBJECTIVES: usize = 20;
/// The longest an objective may allow: the last bound of the histogram, in milliseconds.
pub const MOST_P95_MS: u32 = 30_000;

/// An objective: the 95th percentile of the routes of a group should be at most this, in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Objective {
    /// A group of routes: `desk`, `teams`, `usage`, `evaluation`, `outcomes`, `devices`, `push`,
    /// `federation`, `metrics`, or `other`.
    pub group: String,
    /// The most the group's 95th percentile may be, in milliseconds (1 to 30 000).
    pub p95_ms: u32,
}

/// `metrics` in `config.json`. The section can be left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct MetricsConfig {
    /// The objectives the operator sets for their own node. The answer of `GET /api/metrics` says, for each,
    /// whether it is kept over the range asked for.
    pub objectives: Vec<Objective>,
}

impl MetricsConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Why these settings cannot be used, in words for the owner.
    pub fn check(&self) -> Result<(), String> {
        if self.objectives.len() > MOST_OBJECTIVES {
            return Err(format!(
                "metrics.objectives has more than {MOST_OBJECTIVES} entries"
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for objective in &self.objectives {
            let is_group =
                ROUTE_GROUPS.contains(&objective.group.as_str()) && objective.group != "unmatched";
            if !is_group {
                return Err(format!(
                    "metrics.objectives: \"{}\" is not a group of routes (desk, teams, usage, evaluation, outcomes, devices, push, federation, metrics, other)",
                    objective.group
                ));
            }
            if !(1..=MOST_P95_MS).contains(&objective.p95_ms) {
                return Err(format!(
                    "metrics.objectives: p95Ms of \"{}\" must be 1 to {MOST_P95_MS}",
                    objective.group
                ));
            }
            if !seen.insert(objective.group.as_str()) {
                return Err(format!(
                    "metrics.objectives: \"{}\" has two objectives",
                    objective.group
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(text: &str) -> MetricsConfig {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn the_section_can_be_left_out_and_an_objective_is_a_group_and_a_number() {
        assert!(config("{}").is_default());
        let set =
            config(r#"{"objectives":[{"group":"teams","p95Ms":200},{"group":"desk","p95Ms":50}]}"#);
        assert_eq!(set.objectives.len(), 2);
        assert_eq!(set.check(), Ok(()));
        assert!(serde_json::from_str::<MetricsConfig>(r#"{"objectves":[]}"#).is_err());
        assert!(serde_json::from_str::<MetricsConfig>(
            r#"{"objectives":[{"group":"desk","p95Ms":5,"x":1}]}"#
        )
        .is_err());
        assert!(
            serde_json::from_str::<MetricsConfig>(r#"{"objectives":[{"group":"desk"}]}"#).is_err()
        );
    }

    #[test]
    fn what_cannot_be_an_objective_is_refused_and_names_the_setting() {
        let refused = |text: &str| config(text).check().unwrap_err();
        assert!(
            refused(r#"{"objectives":[{"group":"nothing","p95Ms":100}]}"#)
                .contains("\"nothing\" is not a group")
        );
        assert!(
            refused(r#"{"objectives":[{"group":"unmatched","p95Ms":100}]}"#)
                .contains("not a group"),
            "unmatched is not a group to hold"
        );
        assert!(refused(r#"{"objectives":[{"group":"desk","p95Ms":0}]}"#)
            .contains("p95Ms of \"desk\" must be 1 to 30000"));
        assert!(
            refused(r#"{"objectives":[{"group":"desk","p95Ms":30001}]}"#)
                .contains("must be 1 to 30000")
        );
        assert!(refused(
            r#"{"objectives":[{"group":"desk","p95Ms":10},{"group":"desk","p95Ms":20}]}"#
        )
        .contains("\"desk\" has two objectives"));
        let many: Vec<String> = (0..=MOST_OBJECTIVES)
            .map(|_| r#"{"group":"desk","p95Ms":10}"#.to_owned())
            .collect();
        assert!(
            refused(&format!(r#"{{"objectives":[{}]}}"#, many.join(","))).contains("more than 20")
        );
        for group in [
            "desk",
            "teams",
            "usage",
            "evaluation",
            "outcomes",
            "devices",
            "push",
            "federation",
            "metrics",
            "other",
        ] {
            assert_eq!(
                config(&format!(
                    r#"{{"objectives":[{{"group":"{group}","p95Ms":30000}}]}}"#
                ))
                .check(),
                Ok(()),
                "{group}"
            );
        }
    }
}
