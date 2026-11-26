//! The numbers of the outcome analysis: an observation is one number for one KPI on one day, kept only on the
//! owner's node and read only by the owner (docs/protocol.md, "Outcomes"). The definition holds the KPIs
//! (`outcome.rs`) and never a number. Values are for reading how a measure moves, not for accounting.
use crate::evaluation::check_period;
use crate::ids::Id;
use crate::outcome::Kpi;
use crate::timestamp::{Date, Timestamp};
use crate::usage::valid_days;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The most observations one KPI keeps, and the largest size of a value.
pub const MAX_OBSERVATIONS: usize = 5_000;
pub const MAX_VALUE: f64 = 1e15;
/// A note, and a connector's name, in characters.
pub const NOTE: usize = 200;
pub const SOURCE_NAME: usize = 80;
/// What a history read gives without `limit`, and at most.
pub const DEFAULT_HISTORY: u32 = 100;
pub const MAX_HISTORY: u32 = 500;

/// Where a number came from: entered by hand, or by a connector the owner runs (named). Written as
/// `{ "kind": "manual" }` or `{ "kind": "connector", "name": ... }`; any other field is refused when it is
/// read, since this is read from a file on the node and no signature stands behind it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", try_from = "SourceWire")]
#[ts(export)]
pub enum Source {
    Manual,
    Connector { name: String },
}

/// `Source` as it is written, with the fields a reader may meet, so that one that does not belong is found.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum SourceWire {
    Manual {},
    Connector { name: String },
}

impl TryFrom<SourceWire> for Source {
    type Error = String;
    fn try_from(wire: SourceWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            SourceWire::Manual {} => Self::Manual,
            SourceWire::Connector { name } => Self::Connector { name },
        })
    }
}

/// One number for one KPI on one day. The node sets `entered_at` (its clock) and `entered_by` (the org
/// member id of its account); the one who enters the number sets neither.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct Observation {
    pub kpi: Id,
    /// The calendar day, in the node's time zone, that the number is as of: the last day of a period for a
    /// monthly figure.
    pub date: Date,
    pub value: f64,
    pub source: Source,
    pub entered_at: Timestamp,
    pub entered_by: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What the owner enters: no time and no member, which the node sets. `connector` names a connector's
/// source; without it the number is entered by hand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct ObservationInput {
    pub kpi: Id,
    pub date: Date,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<String>,
}

/// One KPI's numbers over the period.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct KpiSeries {
    pub kpi: Kpi,
    /// The numbers of the period, oldest first, one for each day (the latest entry of that day).
    pub points: Vec<Observation>,
    /// The last number before the period: the baseline a movement is read from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Observation>,
}

/// `GET /api/orgs/<org>/outcomes?days=<n>`: a series for each KPI of the definition, in its order. The days
/// mean what they mean in `Evaluation`, so a screen draws a KPI and the cost on the same days.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Outcomes {
    pub org: Id,
    pub days: u32,
    pub from: Date,
    pub until: Date,
    pub generated_at: Timestamp,
    pub series: Vec<KpiSeries>,
}

/// Whether `value` is a number a KPI may hold: finite and not larger than `MAX_VALUE`.
pub fn valid_value(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAX_VALUE
}

/// Whether `limit` is a number of history entries a read may ask for.
pub fn valid_history_limit(limit: u32) -> bool {
    (1..=MAX_HISTORY).contains(&limit)
}

fn within(what: &str, text: &str, limit: usize) -> Result<(), String> {
    if text.chars().count() > limit || text.chars().any(char::is_control) {
        return Err(format!(
            "{what} is longer than {limit} characters or holds a control character"
        ));
    }
    Ok(())
}

impl Source {
    pub fn check(&self) -> Result<(), String> {
        match self {
            Self::Manual => Ok(()),
            Self::Connector { name } if !name.is_empty() => {
                within("a connector's name", name, SOURCE_NAME)
            }
            Self::Connector { .. } => Err("a connector has a name".into()),
        }
    }
}

impl Observation {
    /// Whether the number, its note and its source are well formed. Not whether its KPI exists or its day is
    /// reasonable: the node knows those.
    pub fn check(&self) -> Result<(), String> {
        if !valid_value(self.value) {
            return Err("a value is finite and at most 1e15 in size".into());
        }
        self.source.check()?;
        self.note
            .as_deref()
            .map_or(Ok(()), |note| within("a note", note, NOTE))
    }
}

impl ObservationInput {
    /// The same checks, before the node makes an `Observation` of it.
    pub fn check(&self) -> Result<(), String> {
        if !valid_value(self.value) {
            return Err("a value is finite and at most 1e15 in size".into());
        }
        if let Some(name) = self.connector.as_deref() {
            Source::Connector {
                name: name.to_owned(),
            }
            .check()?;
        }
        self.note
            .as_deref()
            .map_or(Ok(()), |note| within("a note", note, NOTE))
    }
}

impl Outcomes {
    /// Checks that this is a well-formed answer for the org and range that were asked for: the period is
    /// `days` long, each KPI appears once, and every point is of its KPI, inside the period, in order, and
    /// well formed, with the baseline before the period.
    pub fn check(&self, org: &Id, days: u32) -> Result<(), String> {
        if &self.org != org || self.days != days || !valid_days(days) {
            return Err(format!(
                "reported {} over {} days instead of {org} over {days} days",
                self.org, self.days
            ));
        }
        check_period(&self.from, &self.until, days)?;
        let mut kpis = std::collections::HashSet::new();
        for series in &self.series {
            if !kpis.insert(&series.kpi.id) {
                return Err("reported a KPI twice".into());
            }
            let mut last: Option<&Date> = None;
            for point in &series.points {
                point.check()?;
                let is_in_period = point.date.as_str() >= self.from.as_str()
                    && point.date.as_str() <= self.until.as_str();
                if point.kpi != series.kpi.id || !is_in_period {
                    return Err("reported a point of another KPI or outside the period".into());
                }
                if last.is_some_and(|before| before.as_str() >= point.date.as_str()) {
                    return Err("reported points out of order or twice for a day".into());
                }
                last = Some(&point.date);
            }
            if let Some(before) = &series.before {
                before.check()?;
                if before.kpi != series.kpi.id || before.date.as_str() >= self.from.as_str() {
                    return Err(
                        "reported a baseline of another KPI or not before the period".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const SAMPLE: &str = include_str!("../../../testdata/outcomes.sample.json");

    fn sample() -> Outcomes {
        serde_json::from_str(SAMPLE).unwrap()
    }

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn date(raw: &str) -> Date {
        Date::parse(raw).unwrap()
    }

    #[test]
    fn the_sample_round_trips_and_leaves_out_what_has_no_value() {
        let again = serde_json::to_value(sample()).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        assert!(
            !SAMPLE.contains("null"),
            "an optional field is left out, never null"
        );
        let sample = sample();
        assert!(sample.series[1].before.is_none() && sample.series[0].before.is_some());
        assert_eq!(sample.check(&id("acme"), 14), Ok(()));
    }

    #[test]
    fn what_the_owner_enters_names_no_time_and_no_member() {
        let input: ObservationInput = serde_json::from_value(
            json!({ "kpi": "revenue", "date": "2026-11-24", "value": 1.5, "note": "ok" }),
        )
        .unwrap();
        assert_eq!(input.check(), Ok(()));
        for extra in ["enteredAt", "enteredBy", "source"] {
            let mut value = json!({ "kpi": "revenue", "date": "2026-11-24", "value": 1 });
            value[extra] = json!("owner");
            assert!(
                serde_json::from_value::<ObservationInput>(value).is_err(),
                "{extra}"
            );
        }
        assert!(serde_json::from_value::<ObservationInput>(
            json!({ "kpi": "revenue", "date": "yesterday", "value": 1 })
        )
        .is_err());
    }

    #[test]
    fn a_value_note_and_source_are_checked() {
        let input = |value: f64, note: Option<&str>, connector: Option<&str>| ObservationInput {
            kpi: id("revenue"),
            date: date("2026-11-24"),
            value,
            note: note.map(str::to_owned),
            connector: connector.map(str::to_owned),
        };
        assert_eq!(input(0.0, None, None).check(), Ok(()));
        assert_eq!(
            input(-1e15, None, None).check(),
            Ok(()),
            "a loss is a number too"
        );
        for bad in [f64::NAN, f64::INFINITY, 1.1e15, -1.1e15] {
            assert!(input(bad, None, None).check().is_err(), "{bad}");
        }
        assert!(input(1.0, Some(&"n".repeat(NOTE + 1)), None)
            .check()
            .is_err());
        assert!(input(1.0, Some("line\nbreak"), None).check().is_err());
        assert!(
            input(1.0, None, Some("")).check().is_err(),
            "a connector has a name"
        );
        assert!(input(1.0, None, Some(&"c".repeat(SOURCE_NAME + 1)))
            .check()
            .is_err());
        assert_eq!(input(1.0, None, Some("analytics-export")).check(), Ok(()));
    }

    #[test]
    fn a_malformed_answer_is_refused() {
        let refused = |change: &dyn Fn(&mut Outcomes)| {
            let mut answer = sample();
            change(&mut answer);
            answer.check(&id("acme"), 14).unwrap_err()
        };
        assert!(refused(&|a| a.org = id("other")).contains("instead of"));
        assert!(refused(&|a| a.from = date("2026-11-20")).contains("14 days"));
        assert!(refused(&|a| a.series.push(a.series[0].clone())).contains("twice"));
        assert!(refused(&|a| a.series[0].points[0].kpi = id("activation")).contains("another KPI"));
        assert!(
            refused(&|a| a.series[0].points[0].date = date("2026-12-01"))
                .contains("outside the period")
        );
        assert!(refused(&|a| a.series[1].points.reverse()).contains("out of order"));
        assert!(
            refused(&|a| a.series[1].points[1].date = a.series[1].points[0].date.clone())
                .contains("twice for a day")
        );
        assert!(
            refused(&|a| a.series[0].before.as_mut().unwrap().date = date("2026-11-11"))
                .contains("not before the period")
        );
        assert!(refused(&|a| a.series[0].points[0].value = f64::NAN).contains("finite"));
        assert!(sample().check(&id("acme"), 7).is_err());
        assert!(sample().check(&id("acme"), 91).is_err());
    }

    #[test]
    fn a_connector_is_named_and_a_hand_entry_is_not() {
        assert_eq!(
            serde_json::to_value(Source::Manual).unwrap(),
            json!({ "kind": "manual" })
        );
        let named = Source::Connector { name: "x".into() };
        assert_eq!(
            serde_json::to_value(&named).unwrap(),
            json!({ "kind": "connector", "name": "x" })
        );
        assert!(
            serde_json::from_value::<Source>(json!({ "kind": "manual", "name": "x" })).is_err()
        );
        assert!(valid_history_limit(1) && valid_history_limit(MAX_HISTORY));
        assert!(!valid_history_limit(0) && !valid_history_limit(MAX_HISTORY + 1));
    }
}
