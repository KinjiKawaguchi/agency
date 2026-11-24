//! 鍵 (Ed25519)。秘密鍵は持ち主の台から出さない。公開鍵と指紋を、組織の定義と招待のリンクに書く。
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
    /// 指紋: 公開鍵の SHA-256 の先頭 16 バイトの 16 進 (32 文字)
    pub fn fingerprint(&self) -> String {
        Sha256::digest(self.0.as_bytes())[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
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
            .map_err(|_| "公開鍵が base64 でない".to_owned())?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "公開鍵は 32 バイト".to_owned())?;
        VerifyingKey::from_bytes(&array)
            .map(Self)
            .map_err(|_| "公開鍵として読めない".to_owned())
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

/// アカウントの秘密鍵。保存するときは 32 バイトの種を base64 で書く (ファイルの権限は呼ぶ側が 0600 にする)
pub struct AccountKey(SigningKey);

impl AccountKey {
    pub fn generate() -> Self {
        Self(SigningKey::generate(&mut rand_core::OsRng))
    }

    /// 決まった種から作る (試験の値のため)
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
        let restored = AccountKey::from_encoded(&key.encoded()).expect("読める");
        let signature = restored.sign(b"hello");
        assert!(key.public().verify(b"hello", &signature));
        assert!(!key.public().verify(b"hellO", &signature));
        assert!(!AccountKey::generate().public().verify(b"hello", &signature));
        assert!(!key.public().verify(b"hello", "not base64"));
    }

    #[test]
    fn public_key_text_is_validated() {
        let key = AccountKey::generate().public();
        assert_eq!(PublicKey::try_from(key.encoded()).expect("読める"), key);
        assert!(PublicKey::try_from("AAAA".to_owned()).is_err());
        assert_eq!(key.fingerprint().len(), 32);
    }
}
