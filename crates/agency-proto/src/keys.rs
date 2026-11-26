//! Keys. Account and root keys are Ed25519: a private key never leaves its owner's node; public keys and
//! fingerprints go in org definitions and invite links. A device key is Ed25519 or P-256 (ECDSA with
//! SHA-256), so a phone can keep it in hardware that only does P-256.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use ts_rs::TS;

/// How a P-256 key is written: this prefix, then its uncompressed SEC1 point.
const P256_PREFIX: &str = "p256:";
/// An uncompressed SEC1 point: 0x04, then x and y (32 bytes each).
const P256_LENGTH: usize = 65;

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Ed25519(VerifyingKey),
    /// Checked to be a point on the curve when read.
    P256([u8; P256_LENGTH]),
}

/// A public key. Ed25519: its 32 bytes in standard base64 with padding. P-256: `p256:` and its 65-byte
/// uncompressed point (0x04, x, y) in standard base64 with padding; no other form is read, so one key
/// has one text, one fingerprint, and one device id.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, type = "string")]
pub struct PublicKey(Key);

impl PublicKey {
    /// The key's own bytes: 32 for Ed25519, the 65-byte point for P-256.
    fn bytes(&self) -> &[u8] {
        match &self.0 {
            Key::Ed25519(key) => key.as_bytes(),
            Key::P256(point) => point,
        }
    }

    /// Account and root keys must be Ed25519; only a device key may be P-256.
    pub fn is_ed25519(&self) -> bool {
        matches!(self.0, Key::Ed25519(_))
    }

    /// Fingerprint: the first 16 bytes of the SHA-256 of the key's bytes, as hex (32 characters).
    pub fn fingerprint(&self) -> String {
        Sha256::digest(self.bytes())[..16]
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

    /// Checks a signature in standard base64: Ed25519 (64 bytes), or for P-256 ECDSA with SHA-256 over
    /// the message as 64 bytes `r` then `s`.
    pub fn verify(&self, message: &[u8], signature: &str) -> bool {
        let Ok(bytes) = STANDARD.decode(signature) else {
            return false;
        };
        match &self.0 {
            Key::Ed25519(key) => Signature::from_slice(&bytes)
                .is_ok_and(|signature| key.verify(message, &signature).is_ok()),
            Key::P256(point) => verify_p256(point, message, &bytes),
        }
    }

    pub fn encoded(&self) -> String {
        match &self.0 {
            Key::Ed25519(key) => STANDARD.encode(key.as_bytes()),
            Key::P256(point) => format!("{P256_PREFIX}{}", STANDARD.encode(point)),
        }
    }
}

fn verify_p256(point: &[u8], message: &[u8], signature: &[u8]) -> bool {
    use p256::ecdsa::signature::Verifier as _;
    let Ok(key) = p256::ecdsa::VerifyingKey::from_sec1_bytes(point) else {
        return false;
    };
    p256::ecdsa::Signature::from_slice(signature)
        .is_ok_and(|signature| key.verify(message, &signature).is_ok())
}

fn read_p256(encoded: &str) -> Result<PublicKey, String> {
    let bytes = STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| "the public key is not base64".to_owned())?;
    let point: [u8; P256_LENGTH] = bytes
        .try_into()
        .map_err(|_| "a P-256 key is its 65-byte uncompressed point".to_owned())?;
    if point[0] != 0x04 || p256::ecdsa::VerifyingKey::from_sec1_bytes(&point).is_err() {
        return Err("not a valid P-256 key".into());
    }
    Ok(PublicKey(Key::P256(point)))
}

impl TryFrom<String> for PublicKey {
    type Error = String;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if let Some(encoded) = raw.strip_prefix(P256_PREFIX) {
            return read_p256(encoded);
        }
        let bytes = STANDARD
            .decode(raw.as_bytes())
            .map_err(|_| "the public key is not base64".to_owned())?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "a public key is 32 bytes".to_owned())?;
        VerifyingKey::from_bytes(&array)
            .map(|key| Self(Key::Ed25519(key)))
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

    /// The 32-byte seed, for a key that is written down rather than stored (a recovery key's words).
    pub fn seed(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(Key::Ed25519(self.0.verifying_key()))
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
