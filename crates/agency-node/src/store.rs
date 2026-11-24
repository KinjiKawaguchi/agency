//! 台の中のファイル。書くときは一時ファイルに書いてから置き換え、読む側が書きかけを見ないようにする。
//! 秘密鍵だけは権限を 0600 にする。
use agency_proto::org::SignedOrg;
use agency_proto::{AccountKey, Id};
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::config::NodeConfig;

#[derive(Clone, Debug)]
pub struct DataDir(PathBuf);

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let dir = path.parent().context("置き場のディレクトリが無い")?;
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, serde_json::to_string_pretty(value)? + "\n")?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// ディレクトリの中の .json を全部読む (読めないものは飛ばす)
pub fn read_all<T: DeserializeOwned>(dir: &Path) -> Vec<T> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths.iter().filter_map(|path| read_json(path)).collect()
}

impl DataDir {
    pub fn new(root: PathBuf) -> Self {
        Self(root)
    }

    pub fn root(&self) -> &Path {
        &self.0
    }

    pub fn config_path(&self) -> PathBuf {
        self.0.join("config.json")
    }

    pub fn load_config(&self) -> Result<NodeConfig> {
        read_json(&self.config_path()).with_context(|| {
            format!(
                "{} が読めない (agency init で作る)",
                self.config_path().display()
            )
        })
    }

    pub fn save_config(&self, config: &NodeConfig) -> Result<()> {
        write_json(&self.config_path(), config)
    }

    pub fn key_path(&self) -> PathBuf {
        self.0.join("account.key")
    }

    pub fn load_key(&self) -> Result<AccountKey> {
        let raw = fs::read_to_string(self.key_path()).context("アカウントの鍵が読めない")?;
        AccountKey::from_encoded(&raw).context("アカウントの鍵の形が違う")
    }

    pub fn create_key(&self) -> Result<AccountKey> {
        fs::create_dir_all(&self.0)?;
        fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700))?;
        let key = AccountKey::generate();
        fs::write(self.key_path(), key.encoded() + "\n")?;
        fs::set_permissions(self.key_path(), fs::Permissions::from_mode(0o600))?;
        Ok(key)
    }

    pub fn org_path(&self, org: &Id) -> PathBuf {
        self.0.join("orgs").join(format!("{org}.json"))
    }

    pub fn load_org(&self, org: &Id) -> Option<SignedOrg> {
        read_json(&self.org_path(org))
    }

    /// 組織の定義の写しを置き換える。前の版は履歴として残す
    pub fn save_org(&self, doc: &SignedOrg) -> Result<()> {
        let history = self
            .0
            .join("orgs")
            .join(format!("{}.history", doc.body.org))
            .join(format!("{:06}.json", doc.body.version));
        write_json(&history, doc)?;
        write_json(&self.org_path(&doc.body.org), doc)
    }

    pub fn orgs(&self) -> Vec<SignedOrg> {
        read_all(&self.0.join("orgs"))
    }

    pub fn dir(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
