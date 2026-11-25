//! Keys (Ed25519). A private key never leaves its owner's node; public keys and fingerprints go in org definitions and invite links.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use ts_rs::TS;

/// An Ed25519 public key: 32 bytes in standard base64 with padding.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, type = "string")]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    /// Fingerprint: the first 16 bytes of the public key's SHA-256, as hex (32 characters).
    pub fn fingerprint(&self) -> String {
        Sha256::digest(self.0.as_bytes())[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// The code a person compares on two devices before one adds the other: the fingerprint's first
    /// 12 hex characters in groups of 4 (`3f2a-9c01-77be`). 48 bits, so a node relaying the key cannot
    /// find another key with the same code while the person is pairing.
    pub fn pairing_code(&self) -> String {
        let fingerprint = self.fingerprint();
        [&fingerprint[0..4], &fingerprint[4..8], &fingerprint[8..12]].join("-")
    }

    /// The id a client gives the device that holds this key: `d-` and the pairing code without dashes.
    pub fn device_id(&self) -> crate::ids::Id {
        crate::ids::Id::parse(&format!("d-{}", &self.fingerprint()[..12]))
            .expect("d- and 12 hex characters is an id")
    }

    pub fn verify(&self, message: &[u8], signature: &str) -> bool {
        let Ok(bytes) = STANDARD.decode(signature) else {
            return false;
        };
        let Ok(signature) = Signature::from_slice(&bytes) else {
            return false;
        };
        self.0.verify(message, &signature).is_ok()
    }

    pub fn encoded(&self) -> String {
        STANDARD.encode(self.0.as_bytes())
    }
}

impl TryFrom<String> for PublicKey {
    type Error = String;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let bytes = STANDARD
            .decode(raw.as_bytes())
            .map_err(|_| "the public key is not base64".to_owned())?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "a public key is 32 bytes".to_owned())?;
        VerifyingKey::from_bytes(&array)
            .map(Self)
            .map_err(|_| "not a valid public key".to_owned())
    }
}

impl From<PublicKey> for String {
    fn from(key: PublicKey) -> Self {
        key.encoded()
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.fingerprint())
    }
}

/// An account's private key. It is stored as its 32-byte seed in base64 (the caller makes the file mode 0600).
pub struct AccountKey(SigningKey);

impl AccountKey {
    pub fn generate() -> Self {
        Self(SigningKey::generate(&mut rand_core::OsRng))
    }

    /// From a fixed seed (for test vectors).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&seed))
    }

    pub fn from_encoded(raw: &str) -> Option<Self> {
        let bytes = STANDARD.decode(raw.trim()).ok()?;
        let seed: [u8; 32] = bytes.try_into().ok()?;
        Some(Self::from_seed(seed))
    }

    pub fn encoded(&self) -> String {
        STANDARD.encode(self.0.to_bytes())
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    pub fn sign(&self, message: &[u8]) -> String {
        STANDARD.encode(self.0.sign(message).to_bytes())
    }
}

impl fmt::Debug for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountKey({})", self.public().fingerprint())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_other_keys() {
        let key = AccountKey::generate();
        let restored = AccountKey::from_encoded(&key.encoded()).expect("readable");
        let signature = restored.sign(b"hello");
        assert!(key.public().verify(b"hello", &signature));
        assert!(!key.public().verify(b"hellO", &signature));
        assert!(!AccountKey::generate().public().verify(b"hello", &signature));
        assert!(!key.public().verify(b"hello", "not base64"));
    }

    #[test]
    fn public_key_text_is_validated() {
        let key = AccountKey::generate().public();
        assert_eq!(PublicKey::try_from(key.encoded()).expect("readable"), key);
        assert!(PublicKey::try_from("AAAA".to_owned()).is_err());
        assert_eq!(key.fingerprint().len(), 32);
    }
}
