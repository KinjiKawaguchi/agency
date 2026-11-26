//! How a KPI stood around the day an epic shipped (docs/protocol.md, "Outcomes", "Readings"). Pure
//! arithmetic on the numbers it is given: it reads no store, no clock and no team, so every state has a
//! plain test. It never decides whether a movement is good, and it never says the work caused it.
use crate::delivery::proto_date;
use crate::outcomes::calendar_day;
use agency_proto::clock::{CalendarDate, Duration};
use agency_proto::observation::Observation;
use agency_proto::reading::{
    AimReading, Basis, ReadingState, MIN_POINTS, SETTLE_DAYS, WINDOW_DAYS,
};
use agency_proto::Id;

/// What is known of the day an epic shipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ship {
    /// The epic is not finished.
    NotYet,
    /// It is finished but nobody knows the day.
    Unknown,
    /// The calendar day, in the node's time zone, its last child was finished.
    On(CalendarDate),
}

/// The aim to read: an epic of a team's plan and the KPI it is meant to move.
#[derive(Clone, Copy, Debug)]
pub struct Aim<'a> {
    pub team: &'a Id,
    pub epic: &'a str,
    pub epic_title: Option<&'a str>,
    pub kpi: &'a Id,
    pub ship: Ship,
}

/// Another epic aimed at the same KPI, with the day it shipped.
#[derive(Clone, Copy, Debug)]
pub struct Shipment<'a> {
    pub epic: &'a str,
    pub on: CalendarDate,
}

/// A number and the calendar day it is dated.
type Dated<'a> = (CalendarDate, &'a Observation);

/// Reads `aim` against `numbers`, the numbers that count for its KPI (one for each day, as
/// `outcomes::effective` gives them, in any order), on `today`. `others` are the other aims of the same KPI
/// whose ship day is known.
pub fn read(
    aim: &Aim<'_>,
    numbers: &[Observation],
    today: CalendarDate,
    others: &[Shipment<'_>],
) -> AimReading {
    let ship_day = match aim.ship {
        Ship::NotYet => return refusal(aim, None, ReadingState::NotShipped),
        Ship::Unknown => return refusal(aim, None, ReadingState::ShipDateUnknown),
        Ship::On(day) => day,
    };
    let dated = dated(numbers);
    let window = Duration::days(WINDOW_DAYS);
    let Some(before) = dated
        .iter()
        .rev()
        .find(|(day, _)| *day <= ship_day && *day >= ship_day - window)
    else {
        return refusal(aim, Some(ship_day), ReadingState::NoBaseline);
    };
    let settled = ship_day + Duration::days(SETTLE_DAYS);
    let Some(after) = dated
        .iter()
        .find(|(day, _)| *day >= settled && *day <= ship_day + window)
    else {
        return without_follow_up(aim, ship_day, settled, today);
    };
    let points = dated
        .iter()
        .filter(|(day, _)| (*day - ship_day).abs() <= window)
        .count();
    let points = u32::try_from(points).unwrap_or(u32::MAX);
    if points < MIN_POINTS {
        return AimReading {
            points: Some(points),
            needs: Some(MIN_POINTS),
            ..refusal(aim, Some(ship_day), ReadingState::TooFew)
        };
    }
    ready(aim, ship_day, before, after, others)
}

/// The numbers with a readable date, oldest first.
fn dated(numbers: &[Observation]) -> Vec<Dated<'_>> {
    let mut dated: Vec<Dated<'_>> = numbers
        .iter()
        .filter_map(|one| Some((calendar_day(&one.date)?, one)))
        .collect();
    dated.sort_by_key(|(day, _)| *day);
    dated
}

/// No number at least 14 days after the ship day: too early while the 90 days are not over, and nothing to
/// compare once they are.
fn without_follow_up(
    aim: &Aim<'_>,
    ship_day: CalendarDate,
    settled: CalendarDate,
    today: CalendarDate,
) -> AimReading {
    if today > ship_day + Duration::days(WINDOW_DAYS) {
        return refusal(aim, Some(ship_day), ReadingState::NoFollowUp);
    }
    AimReading {
        readable_from: Some(proto_date(settled)),
        ..refusal(aim, Some(ship_day), ReadingState::TooEarly)
    }
}

fn ready(
    aim: &Aim<'_>,
    ship_day: CalendarDate,
    before: &Dated<'_>,
    after: &Dated<'_>,
    others: &[Shipment<'_>],
) -> AimReading {
    let change = after.1.value - before.1.value;
    let mut between: Vec<&Shipment<'_>> = others
        .iter()
        .filter(|other| other.on >= before.0 && other.on <= after.0)
        .collect();
    between.sort_by_key(|other| (other.on, other.epic));
    AimReading {
        before: Some(before.1.clone()),
        after: Some(after.1.clone()),
        change: Some(change),
        change_ratio: (before.1.value != 0.0).then(|| change / before.1.value),
        overlapping: between.iter().map(|other| other.epic.to_owned()).collect(),
        ..refusal(aim, Some(ship_day), ReadingState::Ready)
    }
}

/// A reading with the aim's names and a state and nothing else.
fn refusal(aim: &Aim<'_>, shipped_on: Option<CalendarDate>, state: ReadingState) -> AimReading {
    AimReading {
        team: aim.team.clone(),
        epic: aim.epic.to_owned(),
        epic_title: aim.epic_title.map(str::to_owned),
        kpi: aim.kpi.clone(),
        shipped_on: shipped_on.map(proto_date),
        state,
        points: None,
        needs: None,
        readable_from: None,
        before: None,
        after: None,
        change: None,
        change_ratio: None,
        overlapping: vec![],
        basis: Basis::Correlation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::observation::Source;
    use agency_proto::{Date, Timestamp};

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn day(raw: &str) -> CalendarDate {
        calendar_day(&Date::parse(raw).unwrap()).unwrap()
    }

    /// The ship day of every test: the numbers are dated by their distance from it.
    fn ship_day() -> CalendarDate {
        day("2026-10-20")
    }

    fn at(days_from_ship: i64) -> CalendarDate {
        ship_day() + Duration::days(days_from_ship)
    }

    fn number(days_from_ship: i64, value: f64) -> Observation {
        Observation {
            kpi: id("activation"),
            date: proto_date(at(days_from_ship)),
            value,
            source: Source::Manual,
            entered_at: Timestamp::parse("2026-11-01T09:00:00Z").unwrap(),
            entered_by: id("owner"),
            note: None,
        }
    }

    fn aim(ship: Ship) -> Aim<'static> {
        // Leaked so that the borrowed ids live as long as the test.
        let team: &'static Id = Box::leak(Box::new(id("app")));
        let kpi: &'static Id = Box::leak(Box::new(id("activation")));
        Aim {
            team,
            epic: "app#10",
            epic_title: Some("Persistent sessions"),
            kpi,
            ship,
        }
    }

    /// Reads, and holds the result to the rules of the type: every reading the function gives is one the
    /// protocol accepts.
    fn read_on(today: i64, numbers: &[Observation], others: &[Shipment<'_>]) -> AimReading {
        let reading = read(&aim(Ship::On(ship_day())), numbers, at(today), others);
        assert_eq!(reading.check(), Ok(()), "{reading:?}");
        assert_eq!(reading.basis, Basis::Correlation);
        reading
    }

    fn state_of(today: i64, numbers: &[Observation]) -> ReadingState {
        read_on(today, numbers, &[]).state
    }

    #[test]
    fn an_epic_that_is_not_finished_or_whose_day_is_unknown_has_nothing_to_compare() {
        let numbers = [number(-5, 40.0), number(20, 44.0), number(40, 45.0)];
        for (ship, state) in [
            (Ship::NotYet, ReadingState::NotShipped),
            (Ship::Unknown, ReadingState::ShipDateUnknown),
        ] {
            let reading = read(&aim(ship), &numbers, at(30), &[]);
            assert_eq!((reading.state, reading.shipped_on.clone()), (state, None));
            assert_eq!(reading.check(), Ok(()));
        }
    }

    #[test]
    fn the_baseline_is_the_last_number_on_or_before_the_ship_day_within_90_days() {
        assert_eq!(state_of(30, &[number(20, 44.0)]), ReadingState::NoBaseline);
        assert_eq!(
            state_of(30, &[number(-91, 40.0), number(20, 44.0)]),
            ReadingState::NoBaseline,
            "91 days before is too old"
        );
        assert_eq!(
            state_of(30, &[number(1, 40.0), number(20, 44.0)]),
            ReadingState::NoBaseline,
            "a number dated after the ship day is not a baseline"
        );
        let numbers = [
            number(-90, 30.0),
            number(-3, 40.0),
            number(0, 41.0),
            number(20, 44.0),
        ];
        let reading = read_on(30, &numbers, &[]);
        assert_eq!(
            reading.before.unwrap().value,
            41.0,
            "the ship day itself counts, and is the last"
        );
        let oldest = read_on(
            30,
            &[number(-90, 30.0), number(20, 44.0), number(25, 45.0)],
            &[],
        );
        assert_eq!(
            oldest.before.unwrap().value,
            30.0,
            "exactly 90 days before counts"
        );
    }

    #[test]
    fn the_follow_up_is_the_first_number_from_14_to_90_days_after() {
        let before = number(-5, 40.0);
        let early = [before.clone(), number(13, 41.0)];
        let reading = read_on(40, &early, &[]);
        assert_eq!(
            reading.state,
            ReadingState::TooEarly,
            "13 days after has not settled"
        );
        assert_eq!(reading.readable_from, Some(proto_date(at(14))));
        let numbers = [
            before.clone(),
            number(13, 41.0),
            number(14, 43.0),
            number(30, 44.0),
        ];
        let reading = read_on(40, &numbers, &[]);
        assert_eq!(
            reading.after.unwrap().value,
            43.0,
            "14 days after is the first that counts"
        );
        let last_day = [before.clone(), number(15, 42.0), number(90, 50.0)];
        assert_eq!(
            read_on(100, &last_day, &[]).after.unwrap().value,
            42.0,
            "the first, not the last"
        );
        let only_last = [before.clone(), number(14, 1.0)];
        assert_eq!(read_on(100, &only_last, &[]).state, ReadingState::TooFew);
        let on_day_90 = [before.clone(), number(-2, 41.0), number(90, 50.0)];
        assert_eq!(
            read_on(100, &on_day_90, &[]).state,
            ReadingState::Ready,
            "90 days after counts"
        );
        let past = [before, number(-2, 41.0), number(91, 50.0)];
        assert_eq!(
            state_of(100, &past),
            ReadingState::NoFollowUp,
            "91 days after is outside the window"
        );
    }

    #[test]
    fn no_follow_up_is_said_only_once_the_90_days_are_over() {
        let numbers = [number(-5, 40.0), number(-2, 41.0), number(3, 42.0)];
        assert_eq!(state_of(0, &numbers), ReadingState::TooEarly);
        assert_eq!(
            state_of(14, &numbers),
            ReadingState::TooEarly,
            "readable, and no number yet"
        );
        assert_eq!(
            state_of(90, &numbers),
            ReadingState::TooEarly,
            "the 90th day can still be entered"
        );
        let late = read_on(91, &numbers, &[]);
        assert_eq!(late.state, ReadingState::NoFollowUp);
        assert_eq!(late.readable_from, None, "there is no day to wait for");
    }

    #[test]
    fn two_numbers_are_a_line_and_not_a_trend() {
        let two = [number(-5, 40.0), number(20, 44.0)];
        let reading = read_on(30, &two, &[]);
        assert_eq!(
            (reading.state, reading.points, reading.needs),
            (ReadingState::TooFew, Some(2), Some(3))
        );
        assert!(reading.before.is_none() && reading.after.is_none() && reading.change.is_none());
        for third in [-90, -40, 5, 60, 90] {
            let three = [two[0].clone(), two[1].clone(), number(third, 42.0)];
            assert_eq!(
                state_of(100, &three),
                ReadingState::Ready,
                "a third number {third} days out"
            );
        }
        for outside in [-91, 91, 200] {
            let three = [two[0].clone(), two[1].clone(), number(outside, 42.0)];
            assert_eq!(
                state_of(300, &three),
                ReadingState::TooFew,
                "{outside} days out is not around it"
            );
        }
    }

    #[test]
    fn a_ready_reading_gives_the_two_numbers_and_what_they_make() {
        let numbers = [number(-5, 40.0), number(-1, 40.0), number(20, 44.0)];
        let reading = read_on(30, &numbers, &[]);
        assert_eq!(reading.state, ReadingState::Ready);
        assert_eq!(reading.shipped_on, Some(proto_date(ship_day())));
        assert_eq!(
            (reading.change, reading.change_ratio),
            (Some(4.0), Some(0.1))
        );
        assert_eq!(reading.epic_title.as_deref(), Some("Persistent sessions"));
        let down = [number(-5, 50.0), number(-1, 50.0), number(20, 40.0)];
        let reading = read_on(30, &down, &[]);
        assert_eq!(
            (reading.change, reading.change_ratio),
            (Some(-10.0), Some(-0.2)),
            "says nothing of good or bad"
        );
    }

    #[test]
    fn a_change_from_nothing_has_no_ratio() {
        let numbers = [number(-5, 0.0), number(-1, 0.0), number(20, 44.0)];
        let reading = read_on(30, &numbers, &[]);
        assert_eq!((reading.change, reading.change_ratio), (Some(44.0), None));
    }

    #[test]
    fn the_order_the_numbers_come_in_does_not_matter() {
        let numbers = [
            number(-5, 40.0),
            number(-1, 41.0),
            number(20, 44.0),
            number(40, 46.0),
        ];
        let sorted = read_on(50, &numbers, &[]);
        let mut shuffled = numbers.to_vec();
        shuffled.reverse();
        shuffled.swap(0, 2);
        assert_eq!(read_on(50, &shuffled, &[]), sorted);
    }

    #[test]
    fn epics_that_shipped_between_the_two_numbers_are_named_in_the_order_they_shipped() {
        let numbers = [number(-5, 40.0), number(-1, 40.0), number(20, 44.0)];
        let others = [
            Shipment {
                epic: "app#30",
                on: at(5),
            },
            Shipment {
                epic: "app#12",
                on: at(5),
            },
            Shipment {
                epic: "app#11",
                on: at(20),
            },
            Shipment {
                epic: "app#13",
                on: at(-1),
            },
            Shipment {
                epic: "app#14",
                on: at(-2),
            },
            Shipment {
                epic: "app#15",
                on: at(21),
            },
        ];
        let reading = read_on(30, &numbers, &others);
        assert_eq!(
            reading.overlapping,
            ["app#13", "app#12", "app#30", "app#11"],
            "from the day of `before` to the day of `after`, both included"
        );
        let refused = read_on(30, &numbers[..1], &others);
        assert!(
            refused.overlapping.is_empty(),
            "only a ready reading names them"
        );
    }
}
