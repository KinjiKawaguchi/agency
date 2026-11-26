//! Root keys kept in files, apart from the data directory: `<dir>/<org>/<fingerprint>.key`, the seed in
//! base64, readable by the owner's OS user only (directories 0700, files 0600). A secret is read only to
//! sign, never at start. This keeps a root key out of copies of the data directory and out of reach of
//! other OS users; a process of the same OS user can still read it (the macOS keychain adapter, with
//! the owner's presence, is what keeps such a process from signing).
use agency_core::ports::RootKeys;
use agency_proto::{AccountKey, Id, PublicKey};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Root keys in files under one directory. The directory may be unknown (no absolute config or home
/// directory): the node still starts, holds no root key, and making or using one answers why.
pub struct FileRootKeys {
    dir: Result<PathBuf, String>,
}

impl FileRootKeys {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: Ok(dir.into()),
        }
    }

    /// Keys under `default_dir()`, or none at all when it is unknown.
    pub fn at_default_dir() -> Self {
        Self {
            dir: Self::default_dir(),
        }
    }

    /// `$XDG_CONFIG_HOME/agency/root-keys`, else `~/.config/agency/root-keys`. Refused when neither
    /// is set to an absolute path, so a root key is never written relative to wherever the node runs.
    pub fn default_dir() -> Result<PathBuf, String> {
        dir_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
    }

    fn path(&self, org: &Id, key: &PublicKey) -> Result<PathBuf, String> {
        let dir = self.dir.as_ref().map_err(Clone::clone)?;
        Ok(dir
            .join(org.as_str())
            .join(format!("{}.key", key.fingerprint())))
    }

    fn read(&self, org: &Id, key: &PublicKey) -> Result<AccountKey, String> {
        let path = self.path(org, key)?;
        let text = fs::read_to_string(&path).map_err(|_| {
            format!(
                "no root key of {org} with fingerprint {}",
                key.fingerprint()
            )
        })?;
        let secret = AccountKey::from_encoded(text.trim())
            .ok_or_else(|| format!("{} is not a root key", path.display()))?;
        if &secret.public() != key {
            return Err(format!("{} holds another key", path.display()));
        }
        Ok(secret)
    }
}

/// The root keys' directory from `XDG_CONFIG_HOME` and `HOME`, of which only absolute paths count.
fn dir_from(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    let absolute = |value: Option<std::ffi::OsString>| {
        value.map(PathBuf::from).filter(|path| path.is_absolute())
    };
    let config = absolute(xdg_config_home)
        .or_else(|| absolute(home).map(|home| home.join(".config")))
        .ok_or("neither XDG_CONFIG_HOME nor HOME is an absolute path, so there is no place for root keys")?;
    Ok(config.join("agency").join("root-keys"))
}

fn private_dir(path: &Path) -> Result<(), String> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|error| format!("cannot make {}: {error}", path.display()))
}

impl RootKeys for FileRootKeys {
    fn create(&self, org: &Id) -> Result<PublicKey, String> {
        let secret = AccountKey::generate();
        let public = secret.public();
        let path = self.path(org, &public)?;
        if let Some(parent) = path.parent() {
            private_dir(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        file.write_all((secret.encoded() + "\n").as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        // The key must survive a power loss before anything is signed with it: after a rotation, peers
        // may hold a version that names it, and it is then the org's only authority.
        if let Some(parent) = path.parent() {
            fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|error| format!("cannot sync {}: {error}", parent.display()))?;
        }
        Ok(public)
    }

    fn holds(&self, org: &Id, key: &PublicKey) -> bool {
        self.path(org, key).is_ok_and(|path| path.is_file())
    }

    fn sign(
        &self,
        org: &Id,
        key: &PublicKey,
        message: &[u8],
        _purpose: &str,
    ) -> Result<String, String> {
        Ok(self.read(org, key)?.sign(message))
    }

    fn forget(&self, org: &Id, key: &PublicKey) -> Result<(), String> {
        let path = self.path(org, key)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("cannot remove {}: {error}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_directory_comes_from_an_absolute_config_or_home_and_never_from_nothing() {
        let some = |text: &str| Some(std::ffi::OsString::from(text));
        assert_eq!(
            dir_from(some("/cfg"), some("/home/alice")).unwrap(),
            PathBuf::from("/cfg/agency/root-keys")
        );
        assert_eq!(
            dir_from(None, some("/home/alice")).unwrap(),
            PathBuf::from("/home/alice/.config/agency/root-keys")
        );
        assert_eq!(
            dir_from(some("relative"), some("/home/alice")).unwrap(),
            PathBuf::from("/home/alice/.config/agency/root-keys"),
            "a relative XDG_CONFIG_HOME is ignored"
        );
        assert!(dir_from(None, None).is_err());
        assert!(
            dir_from(some(""), some("")).is_err(),
            "empty is not a place"
        );
        assert!(
            dir_from(None, some("home")).is_err(),
            "nor is a relative home"
        );
    }

    #[test]
    fn without_a_directory_nothing_is_held_and_making_or_using_a_key_says_why() {
        let keys = FileRootKeys {
            dir: dir_from(None, None),
        };
        let org = Id::parse("acme").unwrap();
        let key = AccountKey::generate().public();
        assert!(!keys.holds(&org, &key));
        let why = keys.create(&org).unwrap_err();
        assert!(why.contains("HOME"), "{why}");
        assert!(keys
            .sign(&org, &key, b"text", "x")
            .unwrap_err()
            .contains("HOME"));
    }

    #[test]
    fn a_key_is_kept_privately_read_only_to_sign_and_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let keys = FileRootKeys::new(dir.path().join("root-keys"));
        let org = Id::parse("acme").unwrap();
        let public = keys.create(&org).unwrap();
        assert!(keys.holds(&org, &public));
        let path = keys.path(&org, &public).unwrap();
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        let signature = keys.sign(&org, &public, b"text", "test").unwrap();
        assert!(public.verify(b"text", &signature));
        let other = Id::parse("other").unwrap();
        assert!(!keys.holds(&other, &public), "kept per org");
        assert!(keys.sign(&other, &public, b"text", "test").is_err());
        keys.forget(&org, &public).unwrap();
        assert!(!keys.holds(&org, &public));
        assert!(keys.sign(&org, &public, b"text", "test").is_err());
    }
}
