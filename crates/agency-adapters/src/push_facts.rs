//! The facts of a notice that a client words in its own language, for APNs and Web Push alike.
use agency_proto::Id;
use serde_json::{json, Value};

/// A recovery's facts: the org, and, when this node keeps a record of the start, when it took it and when the
/// recovery takes effect (ISO 8601).
pub(crate) fn recovery_facts(
    org: &Id,
    started_at: Option<&String>,
    takes_effect_at: Option<&String>,
) -> Value {
    let mut agency = json!({ "type": "recovery", "org": org });
    if let (Some(started), Some(finishes)) = (started_at, takes_effect_at) {
        agency["startedAt"] = json!(started);
        agency["finishesAt"] = json!(finishes);
    }
    agency
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recovery_carries_its_times_only_when_the_node_noted_the_start() {
        let org = Id::parse("acme").unwrap();
        let (started, finishes) = (
            "2026-11-26T09:30:00Z".to_owned(),
            "2026-11-29T09:30:00Z".to_owned(),
        );
        let both = recovery_facts(&org, Some(&started), Some(&finishes));
        assert_eq!(
            both,
            json!({ "type": "recovery", "org": "acme", "startedAt": started, "finishesAt": finishes })
        );
        let none = recovery_facts(&org, None, None);
        assert_eq!(none, json!({ "type": "recovery", "org": "acme" }));
    }
}
