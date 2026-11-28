//! What a `prioritize` decision says: the choice an owner signs for one ticket (docs/protocol.md, "Setting a
//! ticket's priority"). It travels as the `choice` of an envelope's payload, and to the team's runtime as the
//! `priority` of the `prioritize` op. The five priorities are those of a ticket (`tickets::Priority`); `none`
//! clears the board's value.
use crate::tickets::Priority;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The optional operation name for `RuntimeCall::Prioritize` in `Describe::ops`.
pub const PRIORITIZE_OP: &str = "prioritize";

/// A priority to set on a ticket, or `none` (`Clear`) to clear the one it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PriorityChoice {
    Highest,
    High,
    Medium,
    Low,
    Deferred,
    /// Clears the ticket's own priority, so that it takes its epic's again. Written `none`.
    #[serde(rename = "none")]
    Clear,
}

impl PriorityChoice {
    /// The choice an envelope's `choice` says, or `None` for any other text.
    pub fn parse(raw: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(raw.to_owned())).ok()
    }

    /// The priority this sets, or `None` when it clears the ticket's own.
    pub fn priority(self) -> Option<Priority> {
        match self {
            Self::Highest => Some(Priority::Highest),
            Self::High => Some(Priority::High),
            Self::Medium => Some(Priority::Medium),
            Self::Low => Some(Priority::Low),
            Self::Deferred => Some(Priority::Deferred),
            Self::Clear => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, PriorityChoice); 6] = [
        ("highest", PriorityChoice::Highest),
        ("high", PriorityChoice::High),
        ("medium", PriorityChoice::Medium),
        ("low", PriorityChoice::Low),
        ("deferred", PriorityChoice::Deferred),
        ("none", PriorityChoice::Clear),
    ];

    #[test]
    fn the_six_words_are_the_choices() {
        for (word, choice) in ALL {
            assert_eq!(PriorityChoice::parse(word), Some(choice), "{word}");
            assert_eq!(
                serde_json::to_value(choice).unwrap(),
                serde_json::json!(word)
            );
        }
    }

    #[test]
    fn nothing_else_is_a_choice() {
        for raw in ["", "High", "urgent", "none ", "a", "null", "0"] {
            assert_eq!(PriorityChoice::parse(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn each_priority_of_a_ticket_is_one_choice_and_none_clears() {
        let set: Vec<Priority> = ALL
            .iter()
            .filter_map(|(_, choice)| choice.priority())
            .collect();
        assert_eq!(
            set,
            [
                Priority::Highest,
                Priority::High,
                Priority::Medium,
                Priority::Low,
                Priority::Deferred
            ]
        );
        assert_eq!(PriorityChoice::Clear.priority(), None);
    }
}
