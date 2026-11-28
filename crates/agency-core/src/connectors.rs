//! KPIs that take their daily number from the node's own delivery numbers (docs/protocol.md, "Outcomes").
//! Once a day is over, in the node's time zone, the node enters that day's number for each KPI the operator
//! connected (`outcomes.connectors`), with the source `{ kind: "connector", name: "delivery" }`. It never
//! enters a day that already has a number of any source, so a number entered by hand stays and a day is
//! entered once: a later correction of what GitHub said is not written over it.
use crate::config::HostedTeam;
use crate::delivery::{day_numbers, of_team, DayNumbers, Input, Period};
use crate::evaluation::known_daily_costs;
use crate::node::Node;
use crate::outcomes::record_new;
use crate::outcomes_config::{Connector, Metric};
use crate::ports::DeliveryError;
use agency_proto::observation::ObservationInput;
use agency_proto::{Date, Id};
use std::collections::{BTreeMap, BTreeSet};

/// The name a delivery number is entered under: the `name` of the observation's connector source.
pub const SOURCE_NAME: &str = "delivery";
/// How many finished days back a day is entered, the first start's included.
pub const DAYS_BACK: u32 = 30;
/// One day more is read than is entered: what GitHub gave for the first day of a period can miss a red
/// span that began before it, so the oldest day read is never entered.
const MARGIN_DAYS: u32 = 1;

/// What one pass did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    /// The numbers entered.
    pub written: u32,
    /// What went wrong that a later pass may find otherwise (GitHub not answering).
    pub failures: Vec<String>,
    /// What no later pass will change without a new setting or definition.
    pub refused: Vec<String>,
}

/// The day a pass last finished on, which decides whether the node looks again before the day changes: a pass
/// that could not read what it needed is not finished, so the next look is due; one that finished (nothing to
/// retry, though it may have refused a setting) settles the day. What a refusal says is said once, not retried.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled(Option<Date>);

impl Settled {
    /// Whether a pass is due on `today`.
    pub fn is_due(&self, today: &Date) -> bool {
        self.0.as_ref() != Some(today)
    }

    /// Takes in what a pass on `today` came to.
    pub fn after(&mut self, pass: &Pass, today: &Date) {
        if pass.failures.is_empty() {
            self.0 = Some(today.clone());
        }
    }
}

/// Why a KPI cannot take `connector`'s metric, when the org's definition lists the KPI and its unit does not
/// fit. A KPI the definition does not list (yet) is no problem: the number waits for it.
fn unit_mismatch(node: &Node, index: usize, connector: &Connector) -> Option<String> {
    let doc = node.store().load_org(&connector.org)?;
    let kpi = doc.body.kpis.iter().find(|kpi| kpi.id == connector.kpi)?;
    if connector.metric.fits(&kpi.unit) {
        return None;
    }
    Some(format!(
        "outcomes.connectors[{index}].metric: {} is a number of {}, and the KPI {} of {} is not",
        connector.metric.name(),
        connector.metric.unit_words(),
        connector.kpi,
        connector.org
    ))
}

/// The first connector whose metric does not fit its KPI's unit, as a message for the owner naming the
/// setting. The node refuses to start with it.
pub fn check_units(node: &Node) -> Result<(), String> {
    let config = node.config();
    for (index, connector) in config.outcomes.connectors.iter().enumerate() {
        if let Some(message) = unit_mismatch(node, index, connector) {
            return Err(message);
        }
    }
    Ok(())
}

/// A team's day numbers and whether its costs were known.
struct Read {
    numbers: Vec<DayNumbers>,
    is_cost_known: bool,
}

async fn read(node: &Node, hosted: &HostedTeam, days: u32) -> Result<Read, String> {
    let source = hosted
        .delivery_source()
        .ok_or("no repositories are configured for this team")?;
    let period = Period::new(node.now(), node.utc_offset(), days);
    let facts = node
        .delivery()
        .facts(&source, period.start())
        .await
        .map_err(|DeliveryError(reason)| reason)?;
    let costs = known_daily_costs(node, hosted, days).await;
    let pull_requests = of_team(hosted.delivery.as_ref(), facts.pull_requests);
    let input = Input {
        now: node.now(),
        offset: node.utc_offset(),
        days,
        pull_requests: &pull_requests,
        runs: &facts.runs,
        cost_by_day: costs.as_deref().unwrap_or_default(),
    };
    Ok(Read {
        numbers: day_numbers(&input),
        is_cost_known: costs.is_some(),
    })
}

fn value_of(metric: Metric, day: &DayNumbers) -> Option<f64> {
    match metric {
        Metric::NewFeaturesMerged => Some(f64::from(day.new_features_merged)),
        Metric::NewFeaturePrLeadMedianSeconds => {
            day.new_feature_pr_lead_median_seconds.map(|n| n as f64)
        }
        Metric::MainRedSeconds => Some(day.main_red_seconds as f64),
        Metric::CostPerMergedPrUsd => day.cost_per_merged_pr_usd,
        Metric::PrLeadMedianSeconds => day.pr_lead_median_seconds.map(|n| n as f64),
    }
}

/// The finished days of the window that the KPI has no number for, oldest first.
fn empty_days(node: &Node, connector: &Connector, today: &Date) -> Vec<Date> {
    let have: BTreeSet<String> = node
        .store()
        .observations(&connector.org)
        .into_iter()
        .filter(|one| one.kpi == connector.kpi)
        .map(|one| one.date.as_str().to_owned())
        .collect();
    let window = Period::new(node.now(), node.utc_offset(), DAYS_BACK + 1);
    let mut days = vec![];
    let mut day = crate::outcomes::calendar_day(&window.first());
    let today = crate::outcomes::calendar_day(today);
    while let (Some(current), Some(today)) = (day, today) {
        if current >= today {
            break;
        }
        let date = crate::delivery::proto_date(current);
        if !have.contains(date.as_str()) {
            days.push(date);
        }
        day = Some(current + agency_proto::clock::Duration::days(1));
    }
    days
}

/// Enters, for each connector, the numbers of the finished days that have none.
pub async fn write_finished_days(node: &Node) -> Pass {
    let mut pass = Pass::default();
    let config = node.config();
    let today = Period::new(node.now(), node.utc_offset(), 1).last();
    let mut reads: BTreeMap<(Id, Id), Result<Read, String>> = BTreeMap::new();
    for (index, connector) in config.outcomes.connectors.iter().enumerate() {
        if let Some(message) = unit_mismatch(node, index, connector) {
            pass.refused.push(message);
            continue;
        }
        let missing = empty_days(node, connector, &today);
        let Some(hosted) = node.hosted(&connector.org, &connector.team) else {
            continue;
        };
        let is_listed = node
            .store()
            .load_org(&connector.org)
            .is_some_and(|doc| doc.body.kpis.iter().any(|kpi| kpi.id == connector.kpi));
        if missing.is_empty() || !is_listed {
            continue;
        }
        let at = format!("outcomes.connectors[{index}]");
        let key = (connector.org.clone(), connector.team.clone());
        if !reads.contains_key(&key) {
            let days = DAYS_BACK + MARGIN_DAYS + 1;
            reads.insert(key.clone(), read(node, &hosted, days).await);
        }
        let read = match &reads[&key] {
            Ok(read) => read,
            Err(reason) => {
                pass.failures
                    .push(format!("{at}: no numbers this time: {reason}"));
                continue;
            }
        };
        if connector.metric == Metric::CostPerMergedPrUsd && !read.is_cost_known {
            pass.failures.push(format!(
                "{at}: the team's runtime does not report token usage"
            ));
            continue;
        }
        enter(node, connector, &at, read, &missing, &mut pass).await;
    }
    pass
}

async fn enter(
    node: &Node,
    connector: &Connector,
    at: &str,
    read: &Read,
    missing: &[Date],
    pass: &mut Pass,
) {
    for date in missing {
        let day = read.numbers.iter().find(|day| &day.date == date);
        let Some(value) = day.and_then(|day| value_of(connector.metric, day)) else {
            continue;
        };
        let input = ObservationInput {
            kpi: connector.kpi.clone(),
            date: date.clone(),
            value,
            note: None,
            connector: Some(SOURCE_NAME.to_owned()),
        };
        match record_new(node, &connector.org, input).await {
            Ok(Some(_)) => pass.written += 1,
            Ok(None) => {}
            Err(error) => {
                pass.failures
                    .push(format!("{at}: {}: {error}", date.as_str()));
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(raw: &str) -> Date {
        Date::parse(raw).unwrap()
    }

    fn pass(failures: &[&str], refused: &[&str]) -> Pass {
        Pass {
            written: 0,
            failures: failures.iter().map(|one| (*one).to_owned()).collect(),
            refused: refused.iter().map(|one| (*one).to_owned()).collect(),
        }
    }

    #[test]
    fn a_pass_is_due_until_one_has_finished_on_the_day() {
        let (today, tomorrow) = (day("2026-11-24"), day("2026-11-25"));
        let mut settled = Settled::default();
        assert!(settled.is_due(&today), "none has run");

        settled.after(&pass(&["GitHub did not answer"], &[]), &today);
        assert!(
            settled.is_due(&today),
            "a pass that could not read is tried again"
        );

        settled.after(&pass(&[], &[]), &today);
        assert!(!settled.is_due(&today), "a finished one settles the day");
        assert!(settled.is_due(&tomorrow), "and the next day is another");

        settled.after(&pass(&["late failure"], &[]), &tomorrow);
        assert!(
            settled.is_due(&tomorrow),
            "a failure on a new day is not a settled day"
        );
    }

    #[test]
    fn a_refusal_is_not_a_failure_so_it_settles_the_day_too() {
        let today = day("2026-11-24");
        let mut settled = Settled::default();
        settled.after(&pass(&[], &["a unit that does not fit"]), &today);
        assert!(!settled.is_due(&today), "nothing a later look would change");
        let mut both = Settled::default();
        both.after(&pass(&["not read"], &["a unit that does not fit"]), &today);
        assert!(both.is_due(&today), "a failure beside it still retries");
    }
}
