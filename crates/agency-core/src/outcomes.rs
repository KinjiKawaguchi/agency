//! The numbers of the outcome analysis: entered by the owner, kept only on this node, and read only by the
//! owner (docs/protocol.md, "Outcomes"). The org definition says what is measured; the numbers are here. Who
//! may call these is decided by the Client API (every route is owner-only), not here.
use crate::client::membership;
use crate::delivery::Period;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use agency_proto::clock::{CalendarDate, Month};
use agency_proto::observation::{
    valid_history_limit, KpiSeries, Observation, ObservationInput, Outcomes, Source,
    DEFAULT_HISTORY, MAX_OBSERVATIONS,
};
use agency_proto::usage::{valid_days, DEFAULT_USAGE_DAYS, MAX_USAGE_DAYS};
use agency_proto::{Date, Id, Timestamp};
use std::collections::BTreeMap;

/// How far ahead of the node's today a number may be dated, in days, and how far back, in years.
const DAYS_AHEAD: i64 = 1;
const YEARS_BACK: i32 = 10;

fn check_days(days: u32) -> Result<u32> {
    if valid_days(days) {
        Ok(days)
    } else {
        Err(Error::bad_request(format!(
            "days must be from 1 to {MAX_USAGE_DAYS}"
        )))
    }
}

/// The calendar day a `YYYY-MM-DD` date names.
pub(crate) fn calendar_day(date: &Date) -> Option<CalendarDate> {
    let mut parts = date.as_str().split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = Month::try_from(parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    CalendarDate::from_calendar_date(year, month, day).ok()
}

/// Whether `date` is a day an observation may be dated: not more than a day ahead of the node's today, and
/// not before the same day ten years ago (the 29th of February counts as the 28th).
fn check_date(node: &Node, date: &Date) -> Result<()> {
    let today = Period::new(node.now(), node.utc_offset(), 1).last();
    let (Some(day), Some(today)) = (calendar_day(date), calendar_day(&today)) else {
        return Err(Error::bad_request("a malformed date"));
    };
    let oldest =
        CalendarDate::from_calendar_date(today.year() - YEARS_BACK, today.month(), today.day())
            .or_else(|_| {
                CalendarDate::from_calendar_date(
                    today.year() - YEARS_BACK,
                    today.month(),
                    today.day() - 1,
                )
            })
            .map_err(|_| Error::internal("no date ten years back"))?;
    if (day - today).whole_days() > DAYS_AHEAD || day < oldest {
        return Err(Error::bad_request(
            "a number is dated at most a day ahead and at most ten years back",
        ));
    }
    Ok(())
}

/// The numbers that count in a reading: for each KPI and day, the latest one entered (a later entry replaces
/// an earlier, which stays in the history).
pub(crate) fn effective(observations: Vec<Observation>) -> BTreeMap<(Id, String), Observation> {
    let mut latest: BTreeMap<(Id, String), Observation> = BTreeMap::new();
    for one in observations {
        let key = (one.kpi.clone(), one.date.as_str().to_owned());
        let replaces = latest
            .get(&key)
            .is_none_or(|before| before.entered_at.as_str() <= one.entered_at.as_str());
        if replaces {
            latest.insert(key, one);
        }
    }
    latest
}

/// Enters one number: the org must list the KPI and the node's account be a member, the number and its
/// note well formed, the day reasonable, and the KPI below its limit. The node sets the time and the member.
pub async fn record(node: &Node, org: &Id, input: ObservationInput) -> Result<Observation> {
    let written = record_inner(node, org, input, false).await?;
    written.ok_or_else(|| Error::internal("a number was not entered"))
}

/// `record`, for a number a program works out: it is entered only when its KPI has no number at all for that
/// day (entered by hand or not), checked while the node's write lock is held so that a number entered at the
/// same moment is never overwritten. `None` when the day already has one.
pub async fn record_new(
    node: &Node,
    org: &Id,
    input: ObservationInput,
) -> Result<Option<Observation>> {
    record_inner(node, org, input, true).await
}

async fn record_inner(
    node: &Node,
    org: &Id,
    input: ObservationInput,
    is_only_for_an_empty_day: bool,
) -> Result<Option<Observation>> {
    input.check().map_err(Error::bad_request)?;
    let (doc, person) = membership(node, org)?;
    if !doc.body.kpis.iter().any(|kpi| kpi.id == input.kpi) {
        return Err(Error::not_found("not found"));
    }
    check_date(node, &input.date)?;
    let _guard = node.write_lock().lock().await;
    let own: Vec<Observation> = node
        .store()
        .observations(org)
        .into_iter()
        .filter(|one| one.kpi == input.kpi)
        .collect();
    if is_only_for_an_empty_day && own.iter().any(|one| one.date == input.date) {
        return Ok(None);
    }
    if own.len() >= MAX_OBSERVATIONS {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("a KPI keeps at most {MAX_OBSERVATIONS} observations"),
        ));
    }
    let entered_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let source = match input.connector {
        Some(name) => Source::Connector { name },
        None => Source::Manual,
    };
    let observation = Observation {
        kpi: input.kpi,
        date: input.date,
        value: input.value,
        source,
        entered_at,
        entered_by: person.id,
        note: input.note,
    };
    node.store().append_observation(org, &observation)?;
    Ok(Some(observation))
}

/// A series for each KPI of the definition, in its order: the numbers of the last `days` calendar days (the
/// latest entry of each day, oldest first), and the last number before them.
pub fn outcomes(node: &Node, org: &Id, days: Option<u32>) -> Result<Outcomes> {
    let days = check_days(days.unwrap_or(DEFAULT_USAGE_DAYS))?;
    let (doc, _) = membership(node, org)?;
    let period = Period::new(node.now(), node.utc_offset(), days);
    let (from, until) = (period.first(), period.last());
    let counted = effective(node.store().observations(org));
    let series = doc
        .body
        .kpis
        .iter()
        .map(|kpi| {
            let own = counted.values().filter(|one| one.kpi == kpi.id);
            let mut points: Vec<Observation> = vec![];
            let mut before: Option<Observation> = None;
            for one in own {
                if one.date.as_str() < from.as_str() {
                    before = Some(one.clone());
                } else if one.date.as_str() <= until.as_str() {
                    points.push(one.clone());
                }
            }
            KpiSeries {
                kpi: kpi.clone(),
                points,
                before,
            }
        })
        .collect();
    let generated_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    Ok(Outcomes {
        org: org.clone(),
        days,
        from,
        until,
        generated_at,
        series,
    })
}

/// One KPI's history, newest entry first, the ones a later entry replaced included.
pub fn history(node: &Node, org: &Id, kpi: &Id, limit: Option<u32>) -> Result<Vec<Observation>> {
    let limit = limit.unwrap_or(DEFAULT_HISTORY);
    if !valid_history_limit(limit) {
        return Err(Error::bad_request("limit must be from 1 to 500"));
    }
    let (doc, _) = membership(node, org)?;
    if !doc.body.kpis.iter().any(|one| &one.id == kpi) {
        return Err(Error::not_found("not found"));
    }
    let mut own: Vec<Observation> = node
        .store()
        .observations(org)
        .into_iter()
        .filter(|one| &one.kpi == kpi)
        .collect();
    own.reverse();
    own.truncate(limit as usize);
    Ok(own)
}
