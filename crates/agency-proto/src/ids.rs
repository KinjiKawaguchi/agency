//! 識別子。台の境目で形を確かめ、形の違う値は型にしない。
use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("{0}")]
pub struct IdError(pub String);

/// The id of an account, org, team, department, or person: `[a-z0-9][a-z0-9-]{0,39}`
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, type = "string")]
pub struct Id(String);

impl Id {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let bytes = raw.as_bytes();
        let is_head = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
        let is_valid = !bytes.is_empty()
            && bytes.len() <= 40
            && is_head(bytes[0])
            && bytes.iter().all(|&b| is_head(b) || b == b'-');
        if is_valid {
            Ok(Self(raw.to_owned()))
        } else {
            Err(IdError(format!(
                "id の形が違う (英小文字、数字、- で 40 文字まで): {raw}"
            )))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Id {
    type Error = IdError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<Id> for String {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 封筒の id: 16 バイトの乱数の 16 進 (32 文字)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EnvelopeId(String);

impl EnvelopeId {
    /// From 16 random bytes supplied by the caller (a randomness port).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EnvelopeId {
    type Error = IdError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let is_valid = raw.len() == 32
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if is_valid {
            Ok(Self(raw))
        } else {
            Err(IdError(format!("封筒の id の形が違う: {raw}")))
        }
    }
}

impl From<EnvelopeId> for String {
    fn from(id: EnvelopeId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_accepts_only_the_documented_shape() {
        assert!(Id::parse("acme").is_ok());
        assert!(Id::parse("app-mobile-2").is_ok());
        for bad in ["", "-a", "A", "a_b", "../x", &"a".repeat(41)] {
            assert!(Id::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn envelope_id_is_32_lower_hex() {
        assert_eq!(EnvelopeId::from_bytes([0xab; 16]).as_str(), "ab".repeat(16));
        assert!(EnvelopeId::try_from("A".repeat(32)).is_err());
        assert!(EnvelopeId::try_from("ab".to_owned()).is_err());
    }
}
