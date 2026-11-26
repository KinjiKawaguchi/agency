//! Finding a Claude Code login: where Claude Code keeps it for a `CLAUDE_CONFIG_DIR`, and the access
//! token in it. A token lives only in an `AccessToken`, which never prints itself.
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

/// A Claude access token. It has no `Display`, and its `Debug` leaves the token out.
pub struct AccessToken(String);

impl AccessToken {
    /// The token, for the one request that needs it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccessToken(..)")
    }
}

/// An OAuth login as Claude Code stores it.
#[derive(Debug)]
pub struct Login {
    pub token: AccessToken,
    /// When the access token runs out, in milliseconds since the Unix epoch.
    pub expires_at_ms: i64,
}

impl Login {
    /// The login in the text Claude Code stores (`{"claudeAiOauth":{"accessToken":…,"expiresAt":…}}`).
    /// A failure says nothing of the text, which holds secrets.
    pub fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text.trim()).ok()?;
        let oauth = value.get("claudeAiOauth")?;
        let token = oauth
            .get("accessToken")?
            .as_str()
            .filter(|t| !t.is_empty())?;
        Some(Self {
            token: AccessToken(token.to_owned()),
            expires_at_ms: oauth.get("expiresAt")?.as_i64()?,
        })
    }
}

/// Where the logins of one source can be read.
#[async_trait]
pub trait LoginStore: Send + Sync {
    /// Every login found, in no order. Empty when there is none.
    async fn logins(&self) -> Vec<Login>;
    /// Where a person logs in, for the message that says none was found.
    fn describe(&self) -> String;
}

/// The logins of one `CLAUDE_CONFIG_DIR`, or of the default directory.
pub struct ClaudeConfigDir {
    /// As set in `CLAUDE_CONFIG_DIR`; `None` for `~/.claude`.
    dir: Option<PathBuf>,
    home: PathBuf,
    user: String,
    use_keychain: bool,
}

/// The macOS Keychain item of a config directory. Claude Code adds the first 8 hex characters of the
/// SHA-256 of the directory's text to the name, unless `CLAUDE_CONFIG_DIR` is not set at all.
pub fn keychain_service(dir: Option<&Path>) -> String {
    let base = "Claude Code-credentials";
    let Some(dir) = dir else {
        return base.to_owned();
    };
    let digest = Sha256::digest(dir.to_string_lossy().as_bytes());
    let suffix: String = digest
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{base}-{suffix}")
}

impl ClaudeConfigDir {
    pub fn new(dir: Option<PathBuf>, home: PathBuf, user: String) -> Self {
        Self {
            dir,
            home,
            user,
            use_keychain: cfg!(target_os = "macos"),
        }
    }

    #[cfg(test)]
    fn without_keychain(mut self) -> Self {
        self.use_keychain = false;
        self
    }

    /// The file Claude Code keeps a login in on Linux, and on macOS when the Keychain refused it.
    pub fn credentials_file(&self) -> PathBuf {
        let dir = self
            .dir
            .clone()
            .unwrap_or_else(|| self.home.join(".claude"));
        dir.join(".credentials.json")
    }

    fn read_file(&self) -> Option<Login> {
        Login::parse(&std::fs::read_to_string(self.credentials_file()).ok()?)
    }

    /// The item for this account first; any account's item of that name when there is none (Claude
    /// Code names the account after the macOS user, and older logins may differ).
    async fn read_keychain(&self) -> Option<Login> {
        let service = keychain_service(self.dir.as_deref());
        for account in [Some(self.user.as_str()), None] {
            if let Some(login) = read_keychain_item(&service, account).await {
                return Some(login);
            }
        }
        None
    }
}

/// `security find-generic-password -w`: the item's secret goes to this function only, never to the
/// node's output.
async fn read_keychain_item(service: &str, account: Option<&str>) -> Option<Login> {
    let mut command = tokio::process::Command::new("security");
    command.args(["find-generic-password", "-s", service]);
    if let Some(account) = account {
        command.args(["-a", account]);
    }
    command
        .arg("-w")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    // A locked Keychain waits for a person to answer a prompt; the read gives up instead.
    let output = tokio::time::timeout(Duration::from_secs(20), command.output())
        .await
        .ok()?
        .ok()?;
    output
        .status
        .success()
        .then(|| Login::parse(&String::from_utf8_lossy(&output.stdout)))
        .flatten()
}

#[async_trait]
impl LoginStore for ClaudeConfigDir {
    async fn logins(&self) -> Vec<Login> {
        let from_keychain = if self.use_keychain {
            self.read_keychain().await
        } else {
            None
        };
        [from_keychain, self.read_file()]
            .into_iter()
            .flatten()
            .collect()
    }

    fn describe(&self) -> String {
        match &self.dir {
            Some(dir) => format!(
                "run `CLAUDE_CONFIG_DIR={} claude` and log in with /login",
                dir.display()
            ),
            None => "run `claude` and log in with /login".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login_text(token: &str, expires_at: i64) -> String {
        format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{token}","refreshToken":"r","expiresAt":{expires_at}}}}}"#
        )
    }

    #[test]
    fn names_the_keychain_item_after_the_config_directory() {
        assert_eq!(keychain_service(None), "Claude Code-credentials");
        // printf %s /home/alice/.claude-work | shasum -a 256 | cut -c1-8
        assert_eq!(
            keychain_service(Some(Path::new("/home/alice/.claude-work"))),
            "Claude Code-credentials-f0fc2950"
        );
        assert_ne!(
            keychain_service(Some(Path::new("/home/alice/.claude-work/"))),
            keychain_service(Some(Path::new("/home/alice/.claude-work"))),
            "the text of the directory is what counts, so it is written as Claude Code runs with it"
        );
    }

    #[test]
    fn reads_a_login_and_never_shows_its_token() {
        let login = Login::parse(&login_text("sk-ant-oat01-SECRET", 1_800_000_000_000)).unwrap();
        assert_eq!(login.token.expose(), "sk-ant-oat01-SECRET");
        assert_eq!(login.expires_at_ms, 1_800_000_000_000);
        assert!(!format!("{login:?}").contains("SECRET"));
        for bad in [
            "",
            "not json",
            "{}",
            r#"{"claudeAiOauth":{"accessToken":""}}"#,
        ] {
            assert!(Login::parse(bad).is_none(), "{bad}");
        }
    }

    #[tokio::test]
    async fn reads_the_credentials_file_of_the_config_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            ClaudeConfigDir::new(Some(dir.path().into()), "/nowhere".into(), "alice".into())
                .without_keychain();
        assert!(store.logins().await.is_empty());
        std::fs::write(dir.path().join(".credentials.json"), login_text("tok", 5)).unwrap();
        let logins = store.logins().await;
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].expires_at_ms, 5);
    }

    #[tokio::test]
    async fn the_default_login_is_in_the_home_directory() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        std::fs::write(
            home.path().join(".claude/.credentials.json"),
            login_text("tok", 7),
        )
        .unwrap();
        let store =
            ClaudeConfigDir::new(None, home.path().into(), "alice".into()).without_keychain();
        assert_eq!(store.logins().await[0].expires_at_ms, 7);
    }
}
