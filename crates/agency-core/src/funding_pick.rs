//! Which funding source new agent sessions start on. The harness that launches a session asks, and
//! sets the answer's `env` for the session; nothing here is a secret, only where a login lives.
//!
//! Sources are tried in the order of the configuration. One is *under* when its 7-day window is below
//! its threshold; the first under one is picked. Without one, a source whose usage cannot be read is
//! picked (unknown is not over), then the known source with the most room left, and only when every
//! source has reached a limit is there none. A session that is already running keeps its source.
use crate::config::{FundingConfig, Tool, UsageCollector};
use crate::ports::FundingSource;
use agency_proto::client::{UsageWindow, UsageWindowName};
use agency_proto::clock::{self, Duration, OffsetDateTime};
use agency_proto::{Id, Timestamp};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_THRESHOLD: u32 = 80;
/// How old a usage file may be and still count as known: six reads of a collector that goes well.
const FRESH_FOR: Duration = Duration::minutes(30);

/// A source that sessions can run on, and how to run one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub source: Id,
    pub tool: Tool,
    /// What to set for a session that runs on this source. Empty for the default login.
    pub env: BTreeMap<String, String>,
    pub seven_day_threshold: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PickState {
    /// A source is under its threshold.
    Ok,
    /// No source is known to be under its threshold, and this one's usage cannot be read.
    Unknown,
    /// Every readable source is past its threshold; this one has the most room left.
    OverThreshold,
    /// Every source has reached a limit.
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub state: PickState,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub chosen: Option<Listed>,
    pub reason: String,
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn env_of(collector: &UsageCollector) -> BTreeMap<String, String> {
    let (name, path) = match collector {
        UsageCollector::Claude { config_dir } => ("CLAUDE_CONFIG_DIR", config_dir),
        UsageCollector::Codex { codex_home, .. } => ("CODEX_HOME", codex_home),
    };
    path.iter()
        .map(|path| (name.to_owned(), path_text(path)))
        .collect()
}

/// The sources that have a collector, in order, for `tool` or for every tool, with their entries. A
/// threshold outside 1 to 100 is an error: a source that is never under, or never over, is not what
/// anybody meant.
fn entries(
    sources: &[FundingConfig],
    tool: Option<Tool>,
) -> Result<Vec<(&FundingConfig, Listed)>, String> {
    let mut found = vec![];
    for source in sources {
        let Some(collector) = &source.collector else {
            continue;
        };
        if tool.is_some_and(|wanted| wanted != collector.tool()) {
            continue;
        }
        let threshold = source.seven_day_threshold.unwrap_or(DEFAULT_THRESHOLD);
        if !(1..=100).contains(&threshold) {
            return Err(format!(
                "the sevenDayThreshold of \"{}\" must be from 1 to 100",
                source.id
            ));
        }
        let listed = Listed {
            source: source.id.clone(),
            tool: collector.tool(),
            env: env_of(collector),
            seven_day_threshold: threshold,
        };
        found.push((source, listed));
    }
    Ok(found)
}

pub fn list(sources: &[FundingConfig], tool: Option<Tool>) -> Result<Vec<Listed>, String> {
    Ok(entries(sources, tool)?
        .into_iter()
        .map(|(_, listed)| listed)
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Standing {
    Under(u32),
    Over(u32),
    AtLimit,
    Unknown,
}

fn window(windows: &[UsageWindow], name: UsageWindowName, now: OffsetDateTime) -> Option<u32> {
    let found = windows.iter().find(|window| window.name == name)?;
    // A window that has reset since the file was written says nothing of the new one.
    (clock::parse(found.resets_at.as_str())? > now).then_some(found.utilization)
}

fn standing(
    windows: &[UsageWindow],
    read_at: Option<&Timestamp>,
    threshold: u32,
    now: OffsetDateTime,
) -> Standing {
    let fresh = read_at
        .and_then(|read_at| clock::parse(read_at.as_str()))
        .is_some_and(|read_at| now - read_at <= FRESH_FOR);
    let Some(seven_day) = window(windows, UsageWindowName::SevenDay, now).filter(|_| fresh) else {
        return Standing::Unknown;
    };
    let five_hour = window(windows, UsageWindowName::FiveHour, now);
    if seven_day >= 100 || five_hour.is_some_and(|used| used >= 100) {
        Standing::AtLimit
    } else if seven_day < threshold {
        Standing::Under(seven_day)
    } else {
        Standing::Over(seven_day)
    }
}

fn describe(listed: &Listed, standing: Standing) -> String {
    let id = &listed.source;
    match standing {
        Standing::Under(used) | Standing::Over(used) => format!(
            "{id} is at {used}% of its 7-day window (threshold {})",
            listed.seven_day_threshold
        ),
        Standing::AtLimit => format!("{id} has reached a limit"),
        Standing::Unknown => format!("the usage of {id} cannot be read"),
    }
}

/// The source for a new session of `tool`, from the usage the sources report at `now`.
pub fn pick(
    sources: &[FundingConfig],
    usage: &dyn FundingSource,
    tool: Tool,
    now: OffsetDateTime,
) -> Result<Pick, String> {
    let standings: Vec<(Listed, Standing)> = entries(sources, Some(tool))?
        .into_iter()
        .map(|(config, listed)| {
            let (windows, read_at) = usage.usage(config);
            let standing = standing(&windows, read_at.as_ref(), listed.seven_day_threshold, now);
            (listed, standing)
        })
        .collect();
    let describe_all = |upto: usize| {
        let described: Vec<String> = standings[..upto]
            .iter()
            .map(|(listed, standing)| describe(listed, *standing))
            .collect();
        described.join("; ")
    };
    let under = first(&standings, |standing| {
        matches!(standing, Standing::Under(_))
    });
    let unknown = first(&standings, |standing| matches!(standing, Standing::Unknown));
    let (state, index) = match (under, unknown, most_room(&standings)) {
        (Some(index), _, _) => (PickState::Ok, index),
        (None, Some(index), _) => (PickState::Unknown, index),
        (None, None, Some(index)) => (PickState::OverThreshold, index),
        (None, None, None) => {
            let reason = match standings.is_empty() {
                true => format!("no funding source runs {tool:?}"),
                false => describe_all(standings.len()),
            };
            return Ok(Pick {
                state: PickState::None,
                chosen: None,
                reason,
            });
        }
    };
    let (listed, standing) = &standings[index];
    let reason = match state {
        PickState::OverThreshold => {
            let used = match standing {
                Standing::Over(used) => *used,
                _ => 0,
            };
            format!(
                "no source is under its threshold; {} has the most room ({used}% of its 7-day window, threshold {})",
                listed.source, listed.seven_day_threshold
            )
        }
        _ => describe_all(index + 1),
    };
    Ok(Pick {
        state,
        chosen: Some(listed.clone()),
        reason,
    })
}

fn first(standings: &[(Listed, Standing)], wanted: fn(&Standing) -> bool) -> Option<usize> {
    standings.iter().position(|(_, standing)| wanted(standing))
}

/// The readable source past its threshold with the lowest use of its 7-day window; the first of equals.
fn most_room(standings: &[(Listed, Standing)]) -> Option<usize> {
    let over = standings
        .iter()
        .enumerate()
        .filter_map(|(index, (_, standing))| match standing {
            Standing::Over(used) => Some((*used, index)),
            _ => None,
        });
    over.min().map(|(_, index)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::client::FundingKind;
    use std::collections::HashMap;

    const NOW: &str = "2026-11-24T12:00:00Z";
    const FRESH: &str = "2026-11-24T11:45:00Z";
    const STALE: &str = "2026-11-24T11:29:00Z";
    const LATER: &str = "2026-11-27T00:00:00Z";
    const EARLIER: &str = "2026-11-24T11:00:00Z";

    #[derive(Default)]
    struct Usage(HashMap<String, (Vec<UsageWindow>, Option<Timestamp>)>);

    impl FundingSource for Usage {
        fn usage(&self, source: &FundingConfig) -> (Vec<UsageWindow>, Option<Timestamp>) {
            self.0.get(source.id.as_str()).cloned().unwrap_or_default()
        }
    }

    fn window(name: UsageWindowName, used: u32, resets_at: &str) -> UsageWindow {
        UsageWindow {
            name,
            length_hours: name.length_hours(),
            utilization: used,
            resets_at: Timestamp::parse(resets_at).unwrap(),
        }
    }

    fn source(
        id: &str,
        collector: Option<UsageCollector>,
        threshold: Option<u32>,
    ) -> FundingConfig {
        FundingConfig {
            id: Id::parse(id).unwrap(),
            label: id.into(),
            kind: FundingKind::Subscription,
            usage_file: None,
            collector,
            seven_day_threshold: threshold,
        }
    }

    fn claude(id: &str, threshold: Option<u32>) -> FundingConfig {
        let dir = Some(format!("/home/alice/.claude-{id}").into());
        source(
            id,
            Some(UsageCollector::Claude { config_dir: dir }),
            threshold,
        )
    }

    /// A report of `seven_day` percent, written at `read_at`, whose weekly window ends later.
    fn reported(usage: &mut Usage, id: &str, seven_day: u32, read_at: &str) {
        let windows = vec![window(UsageWindowName::SevenDay, seven_day, LATER)];
        usage.0.insert(
            id.into(),
            (windows, Some(Timestamp::parse(read_at).unwrap())),
        );
    }

    fn picked(sources: &[FundingConfig], usage: &Usage) -> Pick {
        pick(sources, usage, Tool::Claude, clock::parse(NOW).unwrap()).unwrap()
    }

    fn source_of(pick: &Pick) -> Option<&str> {
        pick.chosen.as_ref().map(|chosen| chosen.source.as_str())
    }

    #[test]
    fn picks_the_first_source_under_its_threshold() {
        let sources = [claude("a", None), claude("b", None)];
        let mut usage = Usage::default();
        reported(&mut usage, "a", 79, FRESH);
        reported(&mut usage, "b", 0, FRESH);
        assert_eq!(source_of(&picked(&sources, &usage)), Some("a"));

        reported(&mut usage, "a", 80, FRESH);
        let next = picked(&sources, &usage);
        assert_eq!(
            (next.state, source_of(&next)),
            (PickState::Ok, Some("b")),
            "at the threshold is past it"
        );
        assert_eq!(
            next.reason,
            "a is at 80% of its 7-day window (threshold 80); b is at 0% of its 7-day window (threshold 80)"
        );
    }

    #[test]
    fn each_source_has_its_own_threshold() {
        let sources = [claude("a", Some(100)), claude("b", Some(50))];
        let mut usage = Usage::default();
        reported(&mut usage, "a", 99, FRESH);
        reported(&mut usage, "b", 10, FRESH);
        assert_eq!(source_of(&picked(&sources, &usage)), Some("a"));
        reported(&mut usage, "a", 100, FRESH);
        assert_eq!(
            source_of(&picked(&sources, &usage)),
            Some("b"),
            "100 is a limit, whatever the threshold"
        );
    }

    #[test]
    fn goes_on_past_every_threshold_on_the_source_with_the_most_room() {
        let sources = [claude("a", None), claude("b", None), claude("c", None)];
        let mut usage = Usage::default();
        reported(&mut usage, "a", 95, FRESH);
        reported(&mut usage, "b", 85, FRESH);
        reported(&mut usage, "c", 85, FRESH);
        let pick = picked(&sources, &usage);
        assert_eq!(
            (pick.state, source_of(&pick)),
            (PickState::OverThreshold, Some("b")),
            "the first of equals"
        );
        assert_eq!(pick.reason, "no source is under its threshold; b has the most room (85% of its 7-day window, threshold 80)");
    }

    #[test]
    fn stops_only_when_every_source_has_reached_a_limit() {
        let sources = [claude("a", None), claude("b", None)];
        let mut usage = Usage::default();
        reported(&mut usage, "a", 100, FRESH);
        let five_hour = window(UsageWindowName::FiveHour, 100, LATER);
        let weekly = window(UsageWindowName::SevenDay, 10, LATER);
        usage.0.insert(
            "b".into(),
            (
                vec![five_hour, weekly],
                Some(Timestamp::parse(FRESH).unwrap()),
            ),
        );
        let none = picked(&sources, &usage);
        assert_eq!((none.state, none.chosen), (PickState::None, None));
        assert_eq!(none.reason, "a has reached a limit; b has reached a limit");

        let five_hour = window(UsageWindowName::FiveHour, 100, EARLIER);
        let weekly = window(UsageWindowName::SevenDay, 10, LATER);
        usage.0.insert(
            "b".into(),
            (
                vec![five_hour, weekly],
                Some(Timestamp::parse(FRESH).unwrap()),
            ),
        );
        assert_eq!(
            source_of(&picked(&sources, &usage)),
            Some("b"),
            "a 5-hour window that has reset is no limit"
        );
    }

    #[test]
    fn a_source_that_cannot_be_read_is_not_over_but_is_not_known_to_be_under() {
        let sources = [claude("a", None), claude("b", None), claude("c", None)];
        let mut usage = Usage::default();
        reported(&mut usage, "a", 90, FRESH);
        reported(&mut usage, "b", 5, STALE);
        let pick = picked(&sources, &usage);
        assert_eq!(
            (pick.state, source_of(&pick)),
            (PickState::Unknown, Some("b")),
            "30 minutes old is too old"
        );
        reported(&mut usage, "c", 5, FRESH);
        assert_eq!(
            source_of(&picked(&sources, &usage)),
            Some("c"),
            "known to be under comes first"
        );

        let reset = vec![window(UsageWindowName::SevenDay, 5, EARLIER)];
        usage
            .0
            .insert("c".into(), (reset, Some(Timestamp::parse(FRESH).unwrap())));
        assert_eq!(
            source_of(&picked(&sources, &usage)),
            Some("b"),
            "a window that has reset says nothing"
        );
    }

    #[test]
    fn lists_how_to_run_each_source_and_leaves_out_the_ones_without_a_collector() {
        let codex = UsageCollector::Codex {
            codex_home: Some("/home/alice/.codex-2".into()),
            command: None,
        };
        let default_login = source(
            "main",
            Some(UsageCollector::Claude { config_dir: None }),
            None,
        );
        let sources = [
            source("file", None, None),
            default_login,
            claude("a", Some(90)),
            source("x", Some(codex), None),
        ];
        let listed = list(&sources, None).unwrap();
        let summary: Vec<_> = listed
            .iter()
            .map(|l| {
                (
                    l.source.as_str(),
                    l.tool,
                    l.env.iter().next().map(|(k, v)| (k.as_str(), v.as_str())),
                    l.seven_day_threshold,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("main", Tool::Claude, None, 80),
                (
                    "a",
                    Tool::Claude,
                    Some(("CLAUDE_CONFIG_DIR", "/home/alice/.claude-a")),
                    90
                ),
                (
                    "x",
                    Tool::Codex,
                    Some(("CODEX_HOME", "/home/alice/.codex-2")),
                    80
                ),
            ]
        );
        assert_eq!(list(&sources, Some(Tool::Codex)).unwrap().len(), 1);
        assert!(list(&[claude("a", Some(0))], None).is_err());
        assert!(list(&[claude("a", Some(101))], None).is_err());
    }

    #[test]
    fn answers_json_a_harness_can_use() {
        let sources = [claude("a", None)];
        let mut usage = Usage::default();
        reported(&mut usage, "a", 12, FRESH);
        let json = serde_json::to_string(&picked(&sources, &usage)).unwrap();
        assert_eq!(
            json,
            r#"{"state":"ok","source":"a","tool":"claude","env":{"CLAUDE_CONFIG_DIR":"/home/alice/.claude-a"},"sevenDayThreshold":80,"reason":"a is at 12% of its 7-day window (threshold 80)"}"#
        );
        let none = pick(&[], &usage, Tool::Codex, clock::parse(NOW).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_string(&none).unwrap(),
            r#"{"state":"none","reason":"no funding source runs Codex"}"#
        );
    }
}
