//! What a caller of the federation routes learns before it is authenticated: nothing about what this
//! node holds. An unknown org or team, a missing, malformed, stale, or wrong signature, a non-member,
//! and a key that is not the one required all get the same `NotFound` with the same message. Each also
//! spends one signature check, so an answer about something that does not exist takes about as long
//! as one about something that does. The federation routes bypass Cloudflare Access, so anyone can ask.
use crate::error::Error;
use agency_proto::{AccountKey, Kind, PublicKey, Signed};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::OnceLock;

/// The message of every unauthenticated refusal.
pub const MESSAGE: &str = "not found";

pub fn not_found() -> Error {
    Error::not_found(MESSAGE)
}

/// A key that stands in when there is no key to check against. Checking a signature with it costs
/// what checking one with a real Ed25519 key costs; its seed is public, so a check with it never
/// counts as passed.
pub(crate) fn stand_in_key() -> &'static PublicKey {
    static KEY: OnceLock<PublicKey> = OnceLock::new();
    KEY.get_or_init(|| AccountKey::from_seed([0x5a; 32]).public())
}

/// Checks `signed` with `key`. Without a key, it spends the same check on the stand-in key and fails.
pub(crate) fn verify<T: Serialize + DeserializeOwned>(
    signed: &Signed<T>,
    kind: Kind,
    key: Option<&PublicKey>,
) -> Result<(), Error> {
    let is_valid = match key {
        Some(key) => signed.verify(kind, key),
        None => {
            let _ = signed.verify(kind, stand_in_key());
            false
        }
    };
    if is_valid {
        Ok(())
    } else {
        Err(not_found())
    }
}
