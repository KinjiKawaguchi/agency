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

pub struct FileRootKeys {
    dir: PathBuf,
}

impl FileRootKeys {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `$XDG_CONFIG_HOME/agency/root-keys`, else `~/.config/agency/root-keys`.
    pub fn default_dir() -> PathBuf {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
            });
        config.join("agency").join("root-keys")
    }

    fn path(&self, org: &Id, key: &PublicKey) -> PathBuf {
        self.dir
            .join(org.as_str())
            .join(format!("{}.key", key.fingerprint()))
    }

    fn read(&self, org: &Id, key: &PublicKey) -> Result<AccountKey, String> {
        let path = self.path(org, key);
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
        let path = self.path(org, &public);
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
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        Ok(public)
    }

    fn holds(&self, org: &Id, key: &PublicKey) -> bool {
        self.path(org, key).is_file()
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
        let path = self.path(org, key);
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
    fn a_key_is_kept_privately_read_only_to_sign_and_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let keys = FileRootKeys::new(dir.path().join("root-keys"));
        let org = Id::parse("acme").unwrap();
        let public = keys.create(&org).unwrap();
        assert!(keys.holds(&org, &public));
        let path = keys.path(&org, &public);
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
