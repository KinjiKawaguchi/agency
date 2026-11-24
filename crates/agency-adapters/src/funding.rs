//! Funding usage read from a JSON file that another tool writes (for a subscription, the usage
//! endpoint's answer: `{"five_hour": {"utilization": 23.0, "resets_at": "..."}, "seven_day": {...}}`).
//! The node never reads or stores a token. A missing or malformed file means the usage is unknown.
use agency_core::config::FundingConfig;
use agency_core::ports::{Clock, FundingSource};
use agency_proto::client::{UsageWindow, UsageWindowName};
use agency_proto::{clock, Timestamp};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

pub struct UsageFiles {
    clock: Arc<dyn Clock>,
}

impl UsageFiles {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock }
    }
}

/// One window, if it has a utilization between 0 and 100 and an RFC 3339 reset time.
fn window(name: UsageWindowName, raw: &Value) -> Option<UsageWindow> {
    let utilization = raw["utilization"]
        .as_f64()
        .filter(|value| (0.0..=100.0).contains(value))?;
    let resets_at = clock::parse(raw["resets_at"].as_str()?).map(clock::format)?;
    Some(UsageWindow {
        name,
        length_hours: name.length_hours(),
        utilization: utilization.round() as u32,
        resets_at: Timestamp::parse(&resets_at).ok()?,
    })
}

/// The windows in a usage file's contents.
pub fn windows_of(text: &str) -> Vec<UsageWindow> {
    let Ok(raw) = serde_json::from_str::<Value>(text) else {
        return vec![];
    };
    let named = [
        (UsageWindowName::FiveHour, &raw["five_hour"]),
        (UsageWindowName::SevenDay, &raw["seven_day"]),
    ];
    named
        .into_iter()
        .filter_map(|(name, value)| window(name, value))
        .collect()
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

impl FundingSource for UsageFiles {
    fn usage(&self, source: &FundingConfig) -> (Vec<UsageWindow>, Option<Timestamp>) {
        let Some(text) = source.usage_file.as_deref().and_then(read) else {
            return (vec![], None);
        };
        let read_at = Timestamp::parse(&clock::format(self.clock.now())).ok();
        (windows_of(&text), read_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_windows_and_skips_malformed_ones() {
        let text = r#"{"five_hour":{"utilization":23.4,"resets_at":"2026-11-24T15:00:00+00:00"},"seven_day":{"utilization":"x","resets_at":"2026-11-30T00:00:00Z"}}"#;
        let windows = windows_of(text);
        assert_eq!(windows.len(), 1);
        assert_eq!(
            (
                windows[0].name,
                windows[0].utilization,
                windows[0].resets_at.as_str()
            ),
            (UsageWindowName::FiveHour, 23, "2026-11-24T15:00:00Z")
        );
        assert!(windows_of("not json").is_empty());
        assert!(
            windows_of(r#"{"five_hour":{"utilization":140,"resets_at":"2026-11-24T15:00:00Z"}}"#)
                .is_empty(),
            "over 100 percent"
        );
    }
}
