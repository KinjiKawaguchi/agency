//! Root keys in the macOS login keychain, signed with only after the owner's presence (Touch ID or the
//! login password). The seed is a generic password item made by the agency binary, so the keychain lets
//! only that binary read it without the keychain's own prompt; any other program (a script, `security`)
//! gets that prompt. The binary asks for presence before it reads the seed, and reads it only to sign,
//! never at start, so a process of the owner's OS user can neither read the key silently nor sign with
//! it unseen. The Secure Enclave cannot hold it: it holds P-256 keys only, and root keys are Ed25519.
use agency_core::ports::RootKeys;
use agency_proto::{AccountKey, Id, PublicKey};

/// The keychain service the items are kept under; each item's account is `<org>/<fingerprint>`.
pub const SERVICE: &str = "agency root key";

/// Where the seeds are kept: the login keychain, or a stand-in in tests.
pub trait SecretStore: Send + Sync {
    fn put(&self, name: &str, secret: &[u8]) -> Result<(), String>;
    /// Whether an item exists, without reading its secret.
    fn has(&self, name: &str) -> bool;
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

    fn holds(&self, org: &Id, key: &PublicKey) -> bool {
        self.store.has(&name(org, key))
    }

    fn sign(
        &self,
        org: &Id,
        key: &PublicKey,
        message: &[u8],
        purpose: &str,
    ) -> Result<String, String> {
        if !self.holds(org, key) {
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
        Ok(secret.sign(message))
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

    pub struct LoginKeychain;

    impl SecretStore for LoginKeychain {
        fn put(&self, name: &str, secret: &[u8]) -> Result<(), String> {
            set_generic_password(SERVICE, name, secret)
                .map_err(|error| format!("the keychain did not keep the root key: {error}"))
        }

        fn has(&self, name: &str) -> bool {
            let mut search = ItemSearchOptions::new();
            search
                .class(ItemClass::generic_password())
                .service(SERVICE)
                .account(name)
                .load_attributes(true)
                .limit(1);
            search.search().is_ok_and(|found| !found.is_empty())
        }

        fn get(&self, name: &str) -> Result<Vec<u8>, String> {
            get_generic_password(SERVICE, name)
                .map_err(|error| format!("the keychain did not give the root key: {error}"))
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            match delete_generic_password(SERVICE, name) {
                Ok(()) => Ok(()),
                Err(error) if error.code() == -25300 => Ok(()),
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
            let (sender, answer) = mpsc::channel();
            let reply = RcBlock::new(move |success: Bool, _error: *mut NSError| {
                let _ = sender.send(success.as_bool());
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
                Ok(true) => Ok(()),
                Ok(false) => Err("the owner did not confirm".into()),
                Err(_) => Err("nobody answered the prompt in time".into()),
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
        fn has(&self, name: &str) -> bool {
            self.0.lock().unwrap().items.contains_key(name)
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
}
