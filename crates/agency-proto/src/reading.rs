//! How a KPI moved after the epic a team's plan aims at it shipped (docs/protocol.md, "Outcomes"). It is a
//! reading of the owner's own numbers around a day: arithmetic, with a state that refuses when there is too
//! little to read, and a basis that says what it is not. It never says the work caused the movement, and
//! never says whether the movement is good: the KPI's `goodWhen` is on its series.
use crate::ids::Id;
use crate::observation::Observation;
use crate::timestamp::{Date, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How long after the ship day a number must be dated to count as after it: the effect needs time to show.
pub const SETTLE_DAYS: i64 = 14;
/// How far from the ship day a number may be and still be read against it, in days.
pub const WINDOW_DAYS: i64 = 90;
/// The fewest numbers of the KPI in the 180 days around the ship day for a reading to be given. Two points
/// are a line, not a trend.
pub const MIN_POINTS: u32 = 3;

/// Why a reading has the numbers it has, or why it has none. Numbers are given only in `ready`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReadingState {
    /// The epic is not finished, so there is nothing to compare.
    NotShipped,
    /// No ship day is known: the epic is finished but its time is not known, or it is older than what the
    /// harness keeps and the node had not yet remembered it.
    ShipDateUnknown,
    /// No number of the KPI in the 90 days up to the ship day.
    NoBaseline,
    /// No number yet at least 14 days after the ship day; `readableFrom` says the day one can first be read.
    TooEarly,
    /// Fewer than `MIN_POINTS` numbers of the KPI in the 180 days around the ship day; `points` and `needs`.
    TooFew,
    Ready,
}

/// What a reading is: a correlation, never a cause. A type of one value so that nothing can leave it out
/// or say anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Basis {
    Correlation,
}

/// One aim of a team's plan, read: the KPI's numbers around the day the epic shipped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct AimReading {
    pub team: Id,
    /// The epic's item reference, as the aim names it.
    pub epic: String,
    /// The epic's title, when its team's view still lists it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic_title: Option<String>,
    pub kpi: Id,
    /// The calendar day, in the node's time zone, the epic's last child was finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shipped_on: Option<Date>,
    pub state: ReadingState,
    /// With `tooFew`: how many numbers there are, and how many are needed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<u32>,
    /// With `tooEarly`: the first day a number dated then would be an `after`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readable_from: Option<Date>,
    /// With `ready`: the last number on or before the ship day, and the first at least 14 days after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Observation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Observation>,
    /// `after` less `before`, and that over `before` (left out when `before` is 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_ratio: Option<f64>,
    /// The other epics (by item reference) that aim at the same KPI and shipped between `before` and `after`:
    /// their effects cannot be told apart from this one's.
    pub overlapping: Vec<String>,
    pub basis: Basis,
}

/// `GET /api/orgs/<org>/outcomes/readings`: a reading for each aim of each team's plan, in the order of the
/// definition. Owner-only, like every route of the numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Readings {
    pub org: Id,
    pub generated_at: Timestamp,
    pub readings: Vec<AimReading>,
}

fn day(date: &Date) -> Option<time::Date> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    time::Date::parse(date.as_str(), &format).ok()
}

impl AimReading {
    /// Checks that the fields are the ones its state gives: numbers only in `ready`, a count with `tooFew`, a
    /// day with `tooEarly`, a ship day in every state but the two that have none, and, in `ready`, a `change`
    /// that is what the two numbers make and a baseline that is not after the ship day, and an `after` that
    /// is not before it.
    pub fn check(&self) -> Result<(), String> {
        let has_numbers = self.before.is_some()
            || self.after.is_some()
            || self.change.is_some()
            || self.change_ratio.is_some();
        let has_ship_day = self.shipped_on.is_some();
        let (wants_ship_day, wants_numbers) = match self.state {
            ReadingState::NotShipped | ReadingState::ShipDateUnknown => (false, false),
            ReadingState::Ready => (true, true),
            _ => (true, false),
        };
        if has_ship_day != wants_ship_day {
            return Err(format!(
                "{:?} has {}a ship day",
                self.state,
                if wants_ship_day { "" } else { "no " }
            ));
        }
        if has_numbers != wants_numbers {
            return Err("numbers are given in `ready` and only there".into());
        }
        let is_tooearly = self.state == ReadingState::TooEarly;
        let is_toofew = self.state == ReadingState::TooFew;
        if self.readable_from.is_some() != is_tooearly {
            return Err("`readableFrom` goes with `tooEarly` and only with it".into());
        }
        if (self.points.is_some(), self.needs.is_some()) != (is_toofew, is_toofew) {
            return Err("`points` and `needs` go with `tooFew` and only with it".into());
        }
        if is_toofew
            && (self.needs != Some(MIN_POINTS)
                || self.points.is_none_or(|points| points >= MIN_POINTS))
        {
            return Err("`tooFew` has fewer points than the minimum, which it names".into());
        }
        if self.state == ReadingState::Ready {
            self.check_ready()?;
        }
        Ok(())
    }

    fn check_ready(&self) -> Result<(), String> {
        let (Some(before), Some(after), Some(change), Some(shipped)) =
            (&self.before, &self.after, self.change, &self.shipped_on)
        else {
            return Err("`ready` has both numbers, their change, and the ship day".into());
        };
        let (Some(before_day), Some(after_day), Some(ship_day)) =
            (day(&before.date), day(&after.date), day(shipped))
        else {
            return Err("a malformed date".into());
        };
        if before.kpi != self.kpi || after.kpi != self.kpi {
            return Err("a number of another KPI".into());
        }
        let settle = ship_day + time::Duration::days(SETTLE_DAYS);
        let window = time::Duration::days(WINDOW_DAYS);
        if before_day > ship_day
            || before_day < ship_day - window
            || after_day < settle
            || after_day > ship_day + window
        {
            return Err("a number outside the window around the ship day".into());
        }
        if change != after.value - before.value {
            return Err("`change` is not what the two numbers make".into());
        }
        let ratio = (before.value != 0.0).then(|| change / before.value);
        if self.change_ratio != ratio {
            return Err(
                "`changeRatio` is not the change over `before`, or is given for a `before` of 0"
                    .into(),
            );
        }
        Ok(())
    }
}

impl Readings {
    /// Checks that this is the answer for `org` and that every reading holds what its state gives.
    pub fn check(&self, org: &Id) -> Result<(), String> {
        if &self.org != org {
            return Err(format!("reported {} instead of {org}", self.org));
        }
        self.readings.iter().try_for_each(AimReading::check)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const SAMPLE: &str = include_str!("../../../testdata/readings.sample.json");

    fn sample() -> Readings {
        serde_json::from_str(SAMPLE).unwrap()
    }

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn date(raw: &str) -> Date {
        Date::parse(raw).unwrap()
    }

    fn ready() -> AimReading {
        sample().readings[0].clone()
    }

    fn state(reading: &Readings, state: ReadingState) -> AimReading {
        reading
            .readings
            .iter()
            .find(|one| one.state == state)
            .unwrap()
            .clone()
    }

    #[test]
    fn the_sample_round_trips_holds_every_state_and_checks() {
        let again = serde_json::to_value(sample()).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        assert!(
            !SAMPLE.contains("null"),
            "an optional field is left out, never null"
        );
        for one in [
            ReadingState::NotShipped,
            ReadingState::ShipDateUnknown,
            ReadingState::NoBaseline,
            ReadingState::TooEarly,
            ReadingState::TooFew,
            ReadingState::Ready,
        ] {
            assert!(
                sample().readings.iter().any(|reading| reading.state == one),
                "{one:?}"
            );
        }
        assert_eq!(sample().check(&id("acme")), Ok(()));
        assert!(sample().check(&id("other")).is_err());
    }

    #[test]
    fn a_reading_is_a_correlation_and_nothing_else() {
        let mut value = serde_json::to_value(ready()).unwrap();
        assert_eq!(value["basis"], "correlation");
        value["basis"] = json!("cause");
        assert!(
            serde_json::from_value::<AimReading>(value.clone()).is_err(),
            "no other basis exists"
        );
        value.as_object_mut().unwrap().remove("basis");
        assert!(
            serde_json::from_value::<AimReading>(value).is_err(),
            "and it cannot be left out"
        );
    }

    #[test]
    fn numbers_are_given_in_ready_and_only_there() {
        let mut early = state(&sample(), ReadingState::TooEarly);
        early.before = ready().before;
        assert!(
            early.check().is_err(),
            "a reading that refuses gives no number"
        );
        let mut unready = ready();
        unready.state = ReadingState::NoBaseline;
        assert!(unready.check().is_err());
        let mut bare = ready();
        bare.after = None;
        assert!(bare.check().is_err(), "`ready` has both");
    }

    #[test]
    fn each_refusing_state_gives_what_it_says_and_no_more() {
        let readings = sample();
        for one in [ReadingState::NotShipped, ReadingState::ShipDateUnknown] {
            let mut reading = state(&readings, one);
            reading.shipped_on = Some(date("2026-10-20"));
            assert!(reading.check().is_err(), "{one:?} has no ship day");
        }
        for one in [
            ReadingState::NoBaseline,
            ReadingState::TooEarly,
            ReadingState::TooFew,
        ] {
            let mut reading = state(&readings, one);
            reading.shipped_on = None;
            assert!(reading.check().is_err(), "{one:?} has a ship day");
        }
        let mut early = state(&readings, ReadingState::TooEarly);
        early.readable_from = None;
        assert!(
            early.check().is_err(),
            "`tooEarly` says when it can be read"
        );
        let mut baseline = state(&readings, ReadingState::NoBaseline);
        baseline.readable_from = Some(date("2026-12-01"));
        assert!(baseline.check().is_err(), "only `tooEarly` has a day");

        let mut few = state(&readings, ReadingState::TooFew);
        few.points = Some(3);
        assert!(
            few.check().is_err(),
            "three is enough, so it is not `tooFew`"
        );
        few.points = Some(2);
        few.needs = Some(2);
        assert!(few.check().is_err(), "it names the minimum of {MIN_POINTS}");
        few.needs = None;
        assert!(few.check().is_err());
        let mut ready_with_count = ready();
        ready_with_count.points = Some(5);
        assert!(ready_with_count.check().is_err(), "a count is for `tooFew`");
    }

    #[test]
    fn a_change_is_what_the_two_numbers_make() {
        let mut wrong = ready();
        wrong.change = Some(5.0);
        assert!(wrong.check().is_err());
        wrong.change_ratio = Some(5.0 / 40.0);
        assert!(
            wrong.check().is_err(),
            "a change and ratio that agree with each other are still not the two numbers'"
        );
        let mut ratio = ready();
        ratio.change_ratio = Some(0.2);
        assert!(ratio.check().is_err());
        ratio.change_ratio = None;
        assert!(
            ratio.check().is_err(),
            "a ratio is given when `before` is not 0"
        );

        let mut from_zero = ready();
        from_zero.before.as_mut().unwrap().value = 0.0;
        from_zero.change = Some(44.0);
        from_zero.change_ratio = None;
        assert_eq!(from_zero.check(), Ok(()), "a change from 0 has no ratio");
        from_zero.change_ratio = Some(1.0);
        assert!(from_zero.check().is_err());
    }

    #[test]
    fn the_numbers_are_inside_the_window_around_the_ship_day_and_of_the_kpi() {
        let with_dates = |before: &str, after: &str| {
            let mut reading = ready();
            reading.before.as_mut().unwrap().date = date(before);
            reading.after.as_mut().unwrap().date = date(after);
            reading.check()
        };
        assert_eq!(
            with_dates("2026-10-20", "2026-11-03"),
            Ok(()),
            "the ship day itself, and exactly 14 days after"
        );
        assert!(
            with_dates("2026-10-21", "2026-11-10").is_err(),
            "a baseline after the ship day"
        );
        assert!(
            with_dates("2026-10-15", "2026-11-02").is_err(),
            "13 days after has not settled"
        );
        assert_eq!(
            with_dates("2026-07-22", "2027-01-18"),
            Ok(()),
            "90 days before, 90 days after"
        );
        assert!(
            with_dates("2026-07-21", "2026-11-10").is_err(),
            "91 days before"
        );
        assert!(
            with_dates("2026-10-15", "2027-01-19").is_err(),
            "91 days after"
        );
        let mut other = ready();
        other.after.as_mut().unwrap().kpi = id("revenue");
        assert!(other.check().is_err(), "a number of another KPI");
        const {
            assert!(
                SETTLE_DAYS == 14 && WINDOW_DAYS == 90 && MIN_POINTS == 3,
                "the spec says these"
            );
        }
    }
}
