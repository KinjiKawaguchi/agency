//! Root keys in the macOS login keychain, signed with only after the owner's presence (Touch ID or the
//! login password). The seed is a generic password item made by the agency binary, so the keychain lets
//! only that binary read it without the keychain's own prompt; any other program (a script, `security`)
//! gets that prompt. The binary asks for presence before it reads the seed, and reads it only to sign,
//! never at start, so a process of the owner's OS user can neither read the key silently nor sign with
//! it unseen. The Secure Enclave cannot hold it: it holds P-256 keys only, and root keys are Ed25519.
use agency_core::ports::{RootKeys, RootSigner};
use agency_proto::{AccountKey, Id, PublicKey};

/// The keychain service the items are kept under; each item's account is `<org>/<fingerprint>`.
pub const SERVICE: &str = "agency root key";

/// Where the seeds are kept: the login keychain, or a stand-in in tests.
pub trait SecretStore: Send + Sync {
    fn put(&self, name: &str, secret: &[u8]) -> Result<(), String>;
    /// Whether an item exists, without reading its secret; an error when the keychain cannot say (it is
    /// locked, or would not answer), which is not the same as not holding the key.
    fn has(&self, name: &str) -> Result<bool, String>;
    fn get(&self, name: &str) -> Result<Vec<u8>, String>;
    fn delete(&self, name: &str) -> Result<(), String>;
}

/// The owner's confirmation before a root signature, with what is to be signed.
pub trait Presence: Send + Sync {
    fn confirm(&self, reason: &str) -> Result<(), String>;
}

pub struct KeychainRootKeys {
    store: Box<dyn SecretStore>,
    presence: Box<dyn Presence>,
}

fn name(org: &Id, key: &PublicKey) -> String {
    format!("{org}/{}", key.fingerprint())
}

impl KeychainRootKeys {
    pub fn new(store: Box<dyn SecretStore>, presence: Box<dyn Presence>) -> Self {
        Self { store, presence }
    }

    /// The login keychain, with Touch ID or the login password before each signature.
    #[cfg(target_os = "macos")]
    pub fn login() -> Self {
        Self::new(Box::new(macos::LoginKeychain), Box::new(macos::DeviceOwner))
    }
}

impl KeychainRootKeys {
    /// Asks for presence once, saying `purpose`, then reads the seed: only for a key that is here, and only
    /// after the owner confirmed.
    fn unlock(&self, org: &Id, key: &PublicKey, purpose: &str) -> Result<AccountKey, String> {
        if !self.store.has(&name(org, key))? {
            return Err(format!(
                "no root key of {org} with fingerprint {}",
                key.fingerprint()
            ));
        }
        self.presence.confirm(purpose)?;
        let raw = self.store.get(&name(org, key))?;
        let text =
            String::from_utf8(raw).map_err(|_| "the keychain item is not a root key".to_owned())?;
        let secret =
            AccountKey::from_encoded(text.trim()).ok_or("the keychain item is not a root key")?;
        if &secret.public() != key {
            return Err("the keychain item holds another key".into());
        }
        Ok(secret)
    }
}

impl RootKeys for KeychainRootKeys {
    fn needs_recovery(&self) -> bool {
        true
    }

    fn create(&self, org: &Id) -> Result<PublicKey, String> {
        let secret = AccountKey::generate();
        let public = secret.public();
        self.store
            .put(&name(org, &public), secret.encoded().as_bytes())?;
        Ok(public)
    }

    /// Only a key the keychain says it holds: a recovery finishes only where its new root key really is.
    fn holds(&self, org: &Id, key: &PublicKey) -> bool {
        self.store.has(&name(org, key)).unwrap_or(false)
    }

    /// A keychain that cannot answer (locked) may hold the key: the node then still knows it is the root,
    /// and the signature itself says the keychain is locked, instead of every root command saying this node
    /// does not hold the root key.
    fn may_hold(&self, org: &Id, key: &PublicKey) -> bool {
        self.store.has(&name(org, key)).unwrap_or(true)
    }

    fn needs_presence(&self) -> bool {
        true
    }

    fn sign(
        &self,
        org: &Id,
        key: &PublicKey,
        message: &[u8],
        purpose: &str,
    ) -> Result<String, String> {
        let secret = self.unlock(org, key, purpose)?;
        Ok(secret.sign(message))
    }

    fn with_signer(
        &self,
        org: &Id,
        key: &PublicKey,
        purpose: &str,
        work: &mut dyn FnMut(&RootSigner) -> Result<(), String>,
    ) -> Result<(), String> {
        let secret = self.unlock(org, key, purpose)?;
        let signer = |message: &[u8]| Ok(secret.sign(message));
        work(&signer)
    }

    fn forget(&self, org: &Id, key: &PublicKey) -> Result<(), String> {
        self.store.delete(&name(org, key))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{Presence, SecretStore, SERVICE};
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};
    use security_framework::item::{ItemClass, ItemSearchOptions};
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    /// `errSecItemNotFound`: the keychain answered, and holds no such item.
    const ITEM_NOT_FOUND: i32 = -25300;

    pub struct LoginKeychain;

    impl SecretStore for LoginKeychain {
        fn put(&self, name: &str, secret: &[u8]) -> Result<(), String> {
            set_generic_password(SERVICE, name, secret)
                .map_err(|error| format!("the keychain did not keep the root key: {error}"))
        }

        fn has(&self, name: &str) -> Result<bool, String> {
            let mut search = ItemSearchOptions::new();
            search
                .class(ItemClass::generic_password())
                .service(SERVICE)
                .account(name)
                .load_attributes(true)
                .skip_authenticated_items(true)
                .limit(1);
            // No dialog from a search: the node's server asks whether it holds a key, and must never put the
            // keychain's unlock question on the owner's screen.
            match search.search() {
                Ok(found) => Ok(!found.is_empty()),
                Err(error) if error.code() == ITEM_NOT_FOUND => Ok(false),
                Err(error) => Err(format!(
                    "the keychain did not answer (it may be locked): {error}"
                )),
            }
        }

        fn get(&self, name: &str) -> Result<Vec<u8>, String> {
            get_generic_password(SERVICE, name).map_err(|error| {
                format!(
                    "the keychain did not give the root key (was its question refused?): {error}"
                )
            })
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            match delete_generic_password(SERVICE, name) {
                Ok(()) => Ok(()),
                Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
                Err(error) => Err(format!("the keychain did not remove the root key: {error}")),
            }
        }
    }

    /// Touch ID, or the login password where there is no Touch ID, through LocalAuthentication.
    pub struct DeviceOwner;

    /// How long the owner has to answer the prompt.
    const ANSWER_WITHIN: Duration = Duration::from_secs(120);

    impl Presence for DeviceOwner {
        fn confirm(&self, reason: &str) -> Result<(), String> {
            let (sender, answer) = mpsc::channel::<Result<(), String>>();
            let reply = RcBlock::new(move |success: Bool, error: *mut NSError| {
                // LocalAuthentication's own words: a cancelled prompt, no password set, Touch ID locked out.
                // SAFETY: on failure the error is a valid NSError for the duration of the call.
                let why = if error.is_null() {
                    "the owner did not confirm".to_owned()
                } else {
                    unsafe { (*error).localizedDescription() }.to_string()
                };
                let _ = sender.send(if success.as_bool() { Ok(()) } else { Err(why) });
            });
            // SAFETY: the context lives until the answer arrives or the wait ends, and the reply block
            // owns only a channel sender.
            let context = unsafe { LAContext::new() };
            unsafe {
                context.evaluatePolicy_localizedReason_reply(
                    LAPolicy::DeviceOwnerAuthentication,
                    &NSString::from_str(reason),
                    &reply,
                );
            }
            match answer.recv_timeout(ANSWER_WITHIN) {
                Ok(answer) => answer,
                Err(_) => {
                    // The prompt does not stay up unanswered after the node gave up on it.
                    unsafe { context.invalidate() };
                    Err("nobody answered the prompt in time".into())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Recorded {
        items: HashMap<String, Vec<u8>>,
        reads: usize,
        prompts: Vec<String>,
        is_refusing: bool,
        is_locked: bool,
    }

    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<Recorded>>);

    impl SecretStore for Fake {
        fn put(&self, name: &str, secret: &[u8]) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .items
                .insert(name.into(), secret.to_vec());
            Ok(())
        }
        fn has(&self, name: &str) -> Result<bool, String> {
            let recorded = self.0.lock().unwrap();
            if recorded.is_locked {
                return Err("the keychain is locked".into());
            }
            Ok(recorded.items.contains_key(name))
        }
        fn get(&self, name: &str) -> Result<Vec<u8>, String> {
            let mut recorded = self.0.lock().unwrap();
            recorded.reads += 1;
            recorded
                .items
                .get(name)
                .cloned()
                .ok_or_else(|| "none".into())
        }
        fn delete(&self, name: &str) -> Result<(), String> {
            self.0.lock().unwrap().items.remove(name);
            Ok(())
        }
    }

    impl Presence for Fake {
        fn confirm(&self, reason: &str) -> Result<(), String> {
            let mut recorded = self.0.lock().unwrap();
            recorded.prompts.push(reason.into());
            if recorded.is_refusing {
                Err("the owner did not confirm".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn the_secret_is_read_only_after_the_owner_confirms_and_only_to_sign() {
        let fake = Fake::default();
        let keys = KeychainRootKeys::new(Box::new(fake.clone()), Box::new(fake.clone()));
        let org = Id::parse("acme").unwrap();
        let key = keys.create(&org).unwrap();
        assert!(keys.holds(&org, &key));
        assert_eq!(
            fake.0.lock().unwrap().reads,
            0,
            "making and finding a key reads no secret"
        );

        fake.0.lock().unwrap().is_refusing = true;
        assert!(keys
            .sign(&org, &key, b"text", "Sign version 2 of Acme")
            .is_err());
        assert_eq!(
            fake.0.lock().unwrap().reads,
            0,
            "a refused prompt reads no secret"
        );

        fake.0.lock().unwrap().is_refusing = false;
        let signature = keys
            .sign(&org, &key, b"text", "Sign version 2 of Acme")
            .unwrap();
        assert!(key.verify(b"text", &signature));
        let recorded = fake.0.lock().unwrap();
        assert_eq!(recorded.reads, 1);
        assert_eq!(
            recorded.prompts, ["Sign version 2 of Acme"; 2],
            "the prompt says what it signs"
        );
    }

    #[test]
    fn a_key_of_another_org_or_a_forgotten_one_signs_nothing_and_asks_nothing() {
        let fake = Fake::default();
        let keys = KeychainRootKeys::new(Box::new(fake.clone()), Box::new(fake.clone()));
        let (acme, other) = (Id::parse("acme").unwrap(), Id::parse("other").unwrap());
        let key = keys.create(&acme).unwrap();
        assert!(keys.sign(&other, &key, b"text", "x").is_err());
        keys.forget(&acme, &key).unwrap();
        assert!(!keys.holds(&acme, &key));
        assert!(keys.sign(&acme, &key, b"text", "x").is_err());
        assert!(
            fake.0.lock().unwrap().prompts.is_empty(),
            "no prompt for a key that is not here"
        );
    }

    #[test]
    fn a_run_of_signatures_asks_once_and_reads_the_secret_once() {
        let fake = Fake::default();
        let keys = KeychainRootKeys::new(Box::new(fake.clone()), Box::new(fake.clone()));
        let org = Id::parse("acme").unwrap();
        let key = keys.create(&org).unwrap();
        assert!(keys.needs_presence());
        let mut signatures = vec![];
        keys.with_signer(&org, &key, "Sign 2 changes for acme", &mut |sign| {
            signatures.push(sign(b"one")?);
            signatures.push(sign(b"two")?);
            Ok(())
        })
        .unwrap();
        assert!(key.verify(b"one", &signatures[0]) && key.verify(b"two", &signatures[1]));
        let recorded = fake.0.lock().unwrap();
        assert_eq!((recorded.prompts.len(), recorded.reads), (1, 1));
    }

    #[test]
    fn a_locked_keychain_still_holds_the_key_and_signing_says_it_is_locked() {
        let fake = Fake::default();
        let keys = KeychainRootKeys::new(Box::new(fake.clone()), Box::new(fake.clone()));
        let org = Id::parse("acme").unwrap();
        let key = keys.create(&org).unwrap();
        fake.0.lock().unwrap().is_locked = true;
        assert!(keys.may_hold(&org, &key), "the node is still the root");
        assert!(
            !keys.holds(&org, &key),
            "but nothing acts as if the key were surely here"
        );
        let why = keys.sign(&org, &key, b"text", "x").unwrap_err();
        assert!(why.contains("locked"), "{why}");
    }

    #[test]
    fn a_locked_keychain_is_said_so_and_is_not_a_missing_key() {
        let fake = Fake::default();
        let keys = KeychainRootKeys::new(Box::new(fake.clone()), Box::new(fake.clone()));
        let org = Id::parse("acme").unwrap();
        let key = keys.create(&org).unwrap();
        fake.0.lock().unwrap().is_locked = true;
        let why = keys.sign(&org, &key, b"text", "x").unwrap_err();
        assert!(why.contains("locked"), "{why}");
        assert!(
            fake.0.lock().unwrap().prompts.is_empty(),
            "no prompt before the keychain answers"
        );
    }

    /// Against the real login keychain, by hand: `cargo test -p agency-adapters -- --ignored
    /// the_login_keychain`. It makes, finds and removes one item with a random name, and asks for nothing.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn the_login_keychain_keeps_finds_and_forgets_an_item() {
        let keychain = macos::LoginKeychain;
        let name = format!("test/{}", AccountKey::generate().public().fingerprint());
        assert_eq!(keychain.has(&name), Ok(false));
        keychain.put(&name, b"seed").unwrap();
        assert_eq!(keychain.has(&name), Ok(true));
        keychain.delete(&name).unwrap();
        assert_eq!(keychain.has(&name), Ok(false));
    }
}
