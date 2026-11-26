//! The settings of the node's log: how verbose it is and how long its files are kept.

use serde::{Deserialize, Serialize};

/// How much the node writes to its log. A line is written when its level is this one or a more urgent
/// one, so `info` writes `error`, `warn`, and `info`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
}

impl LogLevel {
    pub const ALL: [Self; 4] = [Self::Error, Self::Warn, Self::Info, Self::Debug];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.as_str() == text)
    }
}

/// `logging` in `config.json`. Every field has a default, so the section, or any of its fields, can be
/// left out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct LoggingConfig {
    /// Which lines are written (default `info`).
    pub level: LogLevel,
    /// The size, in MiB, at which the log file is closed and a new one started (1 to 1024; default 20).
    pub max_size_mib: u64,
    /// How many days a closed file is kept (1 to 365; default 14).
    pub max_age_days: u32,
    /// How many closed files are kept at most, the newest first (1 to 1000; default 50).
    pub max_files: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::default(),
            max_size_mib: 20,
            max_age_days: 14,
            max_files: 50,
        }
    }
}

impl LoggingConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Why these settings cannot be used, in words for the owner.
    pub fn check(&self) -> Result<(), String> {
        let in_range = |value: u64, most: u64| (1..=most).contains(&value);
        let wrong = [
            ("maxSizeMib", in_range(self.max_size_mib, 1024), "1 to 1024"),
            (
                "maxAgeDays",
                in_range(self.max_age_days.into(), 365),
                "1 to 365",
            ),
            (
                "maxFiles",
                in_range(self.max_files.into(), 1000),
                "1 to 1000",
            ),
        ]
        .into_iter()
        .find(|(_, is_valid, _)| !is_valid);
        match wrong {
            Some((name, _, range)) => Err(format!("logging.{name} must be {range}")),
            None => Ok(()),
        }
    }

    /// The size at which the file is closed, in bytes.
    pub fn max_bytes(&self) -> u64 {
        self.max_size_mib * 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_section_and_its_fields_can_be_left_out() {
        let none: LoggingConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(none, LoggingConfig::default());
        assert!(none.is_default());
        let some: LoggingConfig =
            serde_json::from_str(r#"{"level":"debug","maxSizeMib":5}"#).unwrap();
        assert_eq!(
            (some.level, some.max_size_mib, some.max_age_days),
            (LogLevel::Debug, 5, 14)
        );
        assert!(!some.is_default());
        assert!(serde_json::from_str::<LoggingConfig>(r#"{"levl":"debug"}"#).is_err());
        assert!(serde_json::from_str::<LoggingConfig>(r#"{"level":"loud"}"#).is_err());
    }

    #[test]
    fn levels_are_ordered_from_urgent_to_verbose_and_parse_by_name() {
        assert!(LogLevel::Error < LogLevel::Warn && LogLevel::Info < LogLevel::Debug);
        for level in LogLevel::ALL {
            assert_eq!(LogLevel::parse(level.as_str()), Some(level));
        }
        assert_eq!(LogLevel::parse("loud"), None);
    }

    #[test]
    fn settings_out_of_range_are_refused_with_the_name() {
        assert_eq!(LoggingConfig::default().check(), Ok(()));
        let with = |edit: fn(&mut LoggingConfig)| {
            let mut config = LoggingConfig::default();
            edit(&mut config);
            config.check()
        };
        assert_eq!(
            with(|c| c.max_size_mib = 0),
            Err("logging.maxSizeMib must be 1 to 1024".into())
        );
        assert_eq!(
            with(|c| c.max_size_mib = 1025),
            Err("logging.maxSizeMib must be 1 to 1024".into())
        );
        assert_eq!(
            with(|c| c.max_age_days = 0),
            Err("logging.maxAgeDays must be 1 to 365".into())
        );
        assert_eq!(
            with(|c| c.max_files = 1001),
            Err("logging.maxFiles must be 1 to 1000".into())
        );
        assert_eq!(with(|c| c.max_files = 1000), Ok(()));
        assert_eq!(LoggingConfig::default().max_bytes(), 20 * 1024 * 1024);
    }
}
