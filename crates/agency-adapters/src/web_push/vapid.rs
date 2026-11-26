//! The node's VAPID key (RFC 8292): a P-256 key pair made on first use and kept in the data directory,
//! and the `Authorization` header that proves to a push service that every message comes from this node.
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::SecretKey;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// How long a signed header is valid: at most 24 hours are allowed.
const LIFETIME_SECONDS: i64 = 12 * 3600;

pub struct Vapid {
    secret: SecretKey,
    public: String,
    key_id: String,
}

impl Vapid {
    /// Reads the key in `path`, or makes one (readable by this account only) when there is none.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            let secret = SecretKey::random(&mut rand_core::OsRng);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|error| {
                    format!("cannot write the VAPID key {}: {error}", path.display())
                })?;
            let encoded = B64.encode(secret.to_bytes()) + "\n";
            file.write_all(encoded.as_bytes()).map_err(|error| {
                format!("cannot write the VAPID key {}: {error}", path.display())
            })?;
        }
        let mode = std::fs::metadata(path)
            .map_err(|error| format!("cannot read the VAPID key {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "the VAPID key {} can be read by other accounts; run `chmod 600` on it",
                path.display()
            ));
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read the VAPID key {}: {error}", path.display()))?;
        let bytes = B64
            .decode(raw.trim())
            .map_err(|_| "the VAPID key file is malformed".to_owned())?;
        let secret = SecretKey::from_slice(&bytes)
            .map_err(|_| "the VAPID key file is malformed".to_owned())?;
        Ok(Self::new(secret))
    }

    pub fn new(secret: SecretKey) -> Self {
        let point = secret.public_key().to_sec1_bytes();
        let fingerprint = Sha256::digest(&point);
        let key_id = fingerprint[..6]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Self {
            public: B64.encode(&point),
            key_id,
            secret,
        }
    }

    /// The public key as a browser takes it (`applicationServerKey`): uncompressed, base64url.
    pub fn public_key(&self) -> &str {
        &self.public
    }

    /// A short fingerprint of the public key: a subscription made with another key is void.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// The `Authorization` header for a push service at `audience` (its origin), asked at `now`.
    pub fn authorization(&self, audience: &str, subject: &str, now: i64) -> String {
        let header = B64.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
        let claims =
            serde_json::json!({ "aud": audience, "exp": now + LIFETIME_SECONDS, "sub": subject });
        let signing_input = format!("{header}.{}", B64.encode(claims.to_string()));
        let signature: Signature = SigningKey::from(&self.secret).sign(signing_input.as_bytes());
        format!(
            "vapid t={signing_input}.{}, k={}",
            B64.encode(signature.to_bytes()),
            self.public
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Verifier;
    use p256::ecdsa::VerifyingKey;

    fn vapid() -> Vapid {
        Vapid::new(SecretKey::from_slice(&[9u8; 32]).unwrap())
    }

    #[test]
    fn the_header_has_the_shape_of_rfc_8292_and_a_signature_that_verifies() {
        let key = vapid();
        let header = key.authorization("https://push.example", "mailto:ops@acme.example", 1_000);
        let (token, k) = header
            .strip_prefix("vapid t=")
            .and_then(|rest| rest.split_once(", k="))
            .expect("vapid t=<jwt>, k=<key>");
        assert_eq!(k, key.public_key());
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let claims: serde_json::Value =
            serde_json::from_slice(&B64.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://push.example");
        assert_eq!(claims["sub"], "mailto:ops@acme.example");
        assert_eq!(claims["exp"], 1_000 + 12 * 3600);
        assert_eq!(
            B64.decode(parts[0]).unwrap(),
            br#"{"typ":"JWT","alg":"ES256"}"#
        );

        let verifying = VerifyingKey::from(SigningKey::from(&key.secret));
        let signature = Signature::from_slice(&B64.decode(parts[2]).unwrap()).unwrap();
        let signed = format!("{}.{}", parts[0], parts[1]);
        assert!(verifying.verify(signed.as_bytes(), &signature).is_ok());
        assert!(
            verifying.verify(b"another input", &signature).is_err(),
            "the signature is of this input"
        );
        assert_eq!(B64.decode(k).unwrap().len(), 65);
    }

    #[test]
    fn a_key_file_that_other_accounts_can_read_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vapid.key");
        Vapid::load_or_create(&path).unwrap();
        for mode in [0o640, 0o604, 0o644, 0o666, 0o660] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            let error = Vapid::load_or_create(&path).err().expect("refused");
            assert!(
                error.contains("chmod 600") && !error.contains("BP"),
                "{mode:o}: {error}"
            );
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            Vapid::load_or_create(&path).is_ok(),
            "read-only to this account is fine"
        );
    }

    #[test]
    fn the_key_is_made_once_kept_private_and_gives_the_same_id_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vapid.key");
        let made = Vapid::load_or_create(&path).unwrap();
        let again = Vapid::load_or_create(&path).unwrap();
        assert_eq!(made.public_key(), again.public_key());
        assert_eq!(made.key_id(), again.key_id());
        assert_eq!(made.key_id().len(), 12);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::write(&path, "not a key\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(Vapid::load_or_create(&path).is_err());
        assert_ne!(vapid().key_id(), made.key_id(), "another key, another id");
    }
}
