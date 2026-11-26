//! The calendar days a report covers, in a node's time zone.
use agency_proto::clock::{CalendarDate, Duration, OffsetDateTime, UtcOffset};
use agency_proto::Date;

/// The last `days` calendar days up to and including the day of `now`, in the time zone `offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    offset: UtcOffset,
    first: CalendarDate,
    last: CalendarDate,
}

pub fn proto_date(day: CalendarDate) -> Date {
    Date::parse(&day.to_string()).expect("a calendar date is written as YYYY-MM-DD")
}

impl Period {
    pub fn new(now: OffsetDateTime, offset: UtcOffset, days: u32) -> Self {
        let last = now.to_offset(offset).date();
        let first = last - Duration::days(i64::from(days.max(1)) - 1);
        Self {
            offset,
            first,
            last,
        }
    }

    pub fn first(&self) -> Date {
        proto_date(self.first)
    }

    pub fn last(&self) -> Date {
        proto_date(self.last)
    }

    /// The instant the period starts: midnight at the start of its first day.
    pub fn start(&self) -> OffsetDateTime {
        self.first.midnight().assume_offset(self.offset)
    }

    /// The instant the period ends: midnight at the end of its last day.
    pub fn end(&self) -> OffsetDateTime {
        (self.last + Duration::days(1))
            .midnight()
            .assume_offset(self.offset)
    }

    pub fn contains(&self, at: OffsetDateTime) -> bool {
        self.start() <= at && at < self.end()
    }

    /// The calendar day of `at` in this period's time zone.
    pub fn day_of(&self, at: OffsetDateTime) -> CalendarDate {
        at.to_offset(self.offset).date()
    }

    /// The seconds of `[from, until)` that fall on each calendar day of the period, in order, left out
    /// when none. What lies outside the period is not counted.
    pub fn seconds_by_day(
        &self,
        from: OffsetDateTime,
        until: OffsetDateTime,
    ) -> Vec<(CalendarDate, u64)> {
        let from = from.max(self.start());
        let until = until.min(self.end());
        let mut seconds = vec![];
        let mut day = self.day_of(from);
        while from < until && day <= self.last {
            let day_start = day.midnight().assume_offset(self.offset);
            let day_end = (day + Duration::days(1))
                .midnight()
                .assume_offset(self.offset);
            let inside = from.max(day_start)..until.min(day_end);
            if inside.start < inside.end {
                seconds.push((
                    day,
                    (inside.end - inside.start).whole_seconds().unsigned_abs(),
                ));
            }
            day += Duration::days(1);
        }
        seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::clock;

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    fn tokyo() -> UtcOffset {
        UtcOffset::from_hms(9, 0, 0).unwrap()
    }

    #[test]
    fn the_period_ends_with_the_day_of_now_in_the_nodes_time_zone() {
        let now = at("2026-11-23T20:00:00Z");
        let utc = Period::new(now, UtcOffset::UTC, 14);
        assert_eq!(
            (utc.first().as_str(), utc.last().as_str()),
            ("2026-11-10", "2026-11-23")
        );
        let local = Period::new(now, tokyo(), 14);
        assert_eq!(
            (local.first().as_str(), local.last().as_str()),
            ("2026-11-11", "2026-11-24"),
            "20:00 UTC is already the next morning in Tokyo"
        );
        assert_eq!(local.start(), at("2026-11-10T15:00:00Z"));
        assert_eq!(local.end(), at("2026-11-24T15:00:00Z"));
        let one = Period::new(now, tokyo(), 1);
        assert_eq!((one.first(), one.last()), (one.last(), one.last()));
    }

    #[test]
    fn a_period_is_exactly_the_days_asked_for_across_a_month_and_a_year() {
        let period = Period::new(at("2027-01-03T12:00:00Z"), UtcOffset::UTC, 5);
        assert_eq!(
            (period.first().as_str(), period.last().as_str()),
            ("2026-12-30", "2027-01-03")
        );
    }

    #[test]
    fn contains_is_closed_at_the_start_and_open_at_the_end() {
        let period = Period::new(at("2026-11-24T12:00:00Z"), UtcOffset::UTC, 2);
        assert!(period.contains(at("2026-11-23T00:00:00Z")));
        assert!(!period.contains(at("2026-11-22T23:59:59Z")));
        assert!(period.contains(at("2026-11-24T23:59:59Z")));
        assert!(!period.contains(at("2026-11-25T00:00:00Z")));
    }

    #[test]
    fn splits_a_span_across_midnight_and_leaves_out_what_is_outside() {
        let period = Period::new(at("2026-11-24T12:00:00Z"), UtcOffset::UTC, 2);
        let split = |from, until| {
            period
                .seconds_by_day(at(from), at(until))
                .into_iter()
                .map(|(day, seconds)| (day.to_string(), seconds))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            split("2026-11-23T23:00:00Z", "2026-11-24T01:30:00Z"),
            [("2026-11-23".into(), 3600), ("2026-11-24".into(), 5400)]
        );
        assert_eq!(
            split("2026-11-24T10:00:00Z", "2026-11-24T10:00:30Z"),
            [("2026-11-24".into(), 30)]
        );
        assert_eq!(
            split("2026-11-20T00:00:00Z", "2026-11-23T00:10:00Z"),
            [("2026-11-23".into(), 600)],
            "the part before the period is not counted"
        );
        assert_eq!(
            split("2026-11-24T23:50:00Z", "2026-11-26T00:00:00Z"),
            [("2026-11-24".into(), 600)]
        );
        assert!(split("2026-11-24T10:00:00Z", "2026-11-24T10:00:00Z").is_empty());
        assert!(
            split("2026-11-24T11:00:00Z", "2026-11-24T10:00:00Z").is_empty(),
            "a span that ends before it starts"
        );
        let local = Period::new(at("2026-11-24T12:00:00Z"), tokyo(), 1);
        assert_eq!(
            local.seconds_by_day(at("2026-11-23T14:00:00Z"), at("2026-11-23T16:00:00Z")),
            [(local.day_of(at("2026-11-24T00:00:00Z")), 3600)],
            "the day starts at 15:00 UTC in Tokyo"
        );
    }
}
