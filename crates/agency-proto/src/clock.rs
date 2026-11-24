//! 時刻は RFC 3339 の UTC で書く。
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn format(at: OffsetDateTime) -> String {
    at.replace_nanosecond(0)
        .unwrap_or(at)
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

pub fn parse(raw: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339).ok()
}
