//! The app that makes a Client API call, as it says in the `x-agency-client` header: `<platform>/<version>`
//! and, when it has one, ` (<build>)`, for example `ios/0.3.0 (202611260418)` (docs/protocol.md, "Client API").
//! It is informational: a request without it, or with one that does not read, is served as before. It holds
//! the app and its release only, never anything about the person or the device.

/// The header's name.
pub const HEADER: &str = "x-agency-client";

/// The longest header value read.
const MAX_LENGTH: usize = 64;
/// The most digits of a version's number.
const MAX_DIGITS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Platform {
    Ios,
    Macos,
    Android,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Macos => "macos",
            Platform::Android => "android",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        [Platform::Ios, Platform::Macos, Platform::Android]
            .into_iter()
            .find(|one| one.name() == raw)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientVersion {
    pub platform: Platform,
    /// `MAJOR.MINOR.PATCH`, as written.
    pub version: String,
    /// The build number, an unsigned 64-bit integer: a TestFlight build is a UTC time to the minute.
    pub build: Option<u64>,
}

fn is_number(raw: &str) -> bool {
    (1..=MAX_DIGITS).contains(&raw.len()) && raw.bytes().all(|byte| byte.is_ascii_digit())
}

/// Reads a header value, or `None` when it is not exactly this form.
pub fn parse(raw: &str) -> Option<ClientVersion> {
    if raw.len() > MAX_LENGTH {
        return None;
    }
    let (app, build) = match raw.split_once(" (") {
        Some((app, rest)) => {
            let digits = rest.strip_suffix(')')?;
            let is_digits = !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit());
            (
                app,
                Some(is_digits.then(|| digits.parse::<u64>().ok()).flatten()?),
            )
        }
        None => (raw, None),
    };
    let (platform, version) = app.split_once('/')?;
    let numbers: Vec<&str> = version.split('.').collect();
    if numbers.len() != 3 || !numbers.iter().all(|one| is_number(one)) {
        return None;
    }
    Some(ClientVersion {
        platform: Platform::parse(platform)?,
        version: version.to_owned(),
        build,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(raw: &str) -> Option<(Platform, &str, Option<u64>)> {
        parse(raw).map(|found| (found.platform, raw_version(raw), found.build))
    }

    /// The version text of a header that was read.
    fn raw_version(raw: &str) -> &str {
        raw.split_once('/')
            .map(|(_, rest)| rest.split(" (").next().unwrap())
            .unwrap()
    }

    #[test]
    fn a_header_in_the_agreed_form_is_read() {
        assert_eq!(
            read("ios/0.3.0 (202611260418)"),
            Some((Platform::Ios, "0.3.0", Some(202_611_260_418)))
        );
        assert_eq!(
            read("macos/12.0.1 (1)"),
            Some((Platform::Macos, "12.0.1", Some(1)))
        );
        assert_eq!(
            read("android/0.1.0"),
            Some((Platform::Android, "0.1.0", None)),
            "the build is optional"
        );
    }

    #[test]
    fn a_build_is_an_unsigned_64_bit_integer() {
        let largest = format!("ios/1.0.0 ({})", u64::MAX);
        assert_eq!(parse(&largest).unwrap().build, Some(u64::MAX));
        assert_eq!(
            parse("ios/1.0.0 (18446744073709551616)"),
            None,
            "one more is too large"
        );
    }

    #[test]
    fn anything_else_is_not_read() {
        for raw in [
            "",
            "ios",
            "ios/",
            "/0.3.0",
            "iOS/0.3.0",
            "windows/0.3.0",
            "ios/0.3",
            "ios/0.3.0.1",
            "ios/0.3.0-beta",
            "ios/0.3.x",
            "ios/-1.0.0",
            "ios/1234567.0.0",
            "ios/0.3.0 ()",
            "ios/0.3.0 (abc)",
            "ios/0.3.0 (-1)",
            "ios/0.3.0 (12",
            "ios/0.3.0 (12) ",
            "ios/0.3.0  (12)",
            " ios/0.3.0",
            "ios/0.3.0\n",
            "ios/0.3.0 (1) (2)",
        ] {
            assert_eq!(parse(raw), None, "{raw:?}");
        }
        assert_eq!(
            parse(&format!("ios/0.3.0 ({})", "1".repeat(60))),
            None,
            "longer than the header may be"
        );
    }
}
