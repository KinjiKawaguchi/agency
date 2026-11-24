//! ノードの共有の状態。設定、アカウントの鍵、ハーネス、台どうしの HTTP。
use crate::config::NodeConfig;
use crate::runtime::Runtimes;
use crate::store::DataDir;
use agency_proto::{AccountKey, Id};
use anyhow::Result;
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub struct Node {
    pub data: DataDir,
    config: RwLock<NodeConfig>,
    pub key: AccountKey,
    pub runtimes: Runtimes,
    pub http: reqwest::Client,
    /// 組織の定義、招待、当てた封筒の記録を書くときに順番にする (書き手は 1 つ)
    pub writes: tokio::sync::Mutex<()>,
}

impl Node {
    pub fn open(data: DataDir, runtimes: Option<Runtimes>) -> Result<Arc<Self>> {
        let config = data.load_config()?;
        let key = data.load_key()?;
        let runtimes = runtimes.unwrap_or_else(Runtimes::command);
        let http = reqwest::Client::builder().timeout(Duration::from_secs(8)).build()?;
        Ok(Arc::new(Self { data, config: RwLock::new(config), key, runtimes, http, writes: tokio::sync::Mutex::new(()) }))
    }

    pub fn config(&self) -> NodeConfig {
        self.config.read().map(|one| one.clone()).unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// 設定を書き換えて保存する
    pub fn update_config(&self, edit: impl FnOnce(&mut NodeConfig)) -> Result<()> {
        let mut guard = self.config.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        edit(&mut guard);
        self.data.save_config(&guard)
    }

    pub fn account(&self) -> Id {
        self.config().account.id
    }

    pub fn url(&self) -> String {
        self.config().url
    }
}
