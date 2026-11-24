//! Points in time and calendar dates as they travel in JSON. Malformed values never become these types.
use crate::clock;
use crate::ids::IdError;
use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// RFC 3339 time in UTC, for example `2026-11-24T12:00:00Z`. Fractional seconds are allowed.
/// The original text is kept, so values pass through the node unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, type = "string")]
pub struct Timestamp(String);

impl Timestamp {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let is_utc = raw.ends_with('Z');
        if is_utc && clock::parse(raw).is_some() {
            Ok(Self(raw.to_owned()))
        } else {
            Err(IdError(format!("not an RFC 3339 UTC time: {raw}")))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Timestamp {
    type Error = IdError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<Timestamp> for String {
    fn from(at: Timestamp) -> Self {
        at.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A calendar date, `YYYY-MM-DD`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, type = "string")]
pub struct Date(String);

impl Date {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let format = time::macros::format_description!("[year]-[month]-[day]");
        if raw.len() == 10 && time::Date::parse(raw, &format).is_ok() {
            Ok(Self(raw.to_owned()))
        } else {
            Err(IdError(format!("not a YYYY-MM-DD date: {raw}")))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Date {
    type Error = IdError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<Date> for String {
    fn from(date: Date) -> Self {
        date.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_accepts_only_utc_rfc3339() {
        assert!(Timestamp::parse("2026-11-24T12:00:00Z").is_ok());
        assert!(Timestamp::parse("2026-11-24T12:00:00.123Z").is_ok());
        for bad in ["", "2026-11-24", "2026-11-24T21:00:00+09:00", "2026-13-01T00:00:00Z", "yesterday"] {
            assert!(Timestamp::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn date_accepts_only_calendar_dates() {
        assert!(Date::parse("2026-11-24").is_ok());
        for bad in ["", "2026-11-24T00:00:00Z", "2026-02-30", "26-11-24", "2026-1-24", "+2026-11-24", "-0001-11-24"] {
            assert!(Date::parse(bad).is_err(), "{bad}");
        }
    }
}
