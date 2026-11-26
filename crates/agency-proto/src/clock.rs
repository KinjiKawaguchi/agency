//! Times are written as RFC 3339 in UTC. Reading the current time is the caller's job (a clock port),
//! so nothing here depends on the wall clock.
use time::format_description::well_known::Rfc3339;
pub use time::{Duration, OffsetDateTime};

pub fn format(at: OffsetDateTime) -> String {
    at.replace_nanosecond(0)
        .unwrap_or(at)
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// `format` with milliseconds, for a signed time that should differ between two signatures made in
/// the same second (a signed read is accepted once).
pub fn format_millis(at: OffsetDateTime) -> String {
    let millis = at.millisecond();
    let whole = format(at);
    match whole.strip_suffix('Z') {
        Some(seconds) => format!("{seconds}.{millis:03}Z"),
        None => whole,
    }
}

pub fn parse(raw: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339).ok()
}
