//! Reads each funding source's usage on a schedule and writes its usage file, which the funding
//! report reads (`funding.rs`). A source that names a collector in `config.json` is read here; a source
//! with only a `usageFile` is kept by some other tool.
//!
//! Tokens: a collector reads a login's access token when it needs it, sends it to the one service it
//! belongs to, and keeps it nowhere. It is not written to a file, a log line, or a failure message.
mod claude;
mod claude_login;
mod codex;
mod schedule;
mod usage_file;

use agency_core::config::{FundingConfig, UsageCollector};
use agency_core::ports::Clock;
use agency_core::Node;
use async_trait::async_trait;
use claude::{ClaudeUsage, USAGE_URL};
use claude_login::ClaudeConfigDir;
use codex::CodexUsage;
use schedule::{wait_after_failure, Failure, INTERVAL};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

/// Where one source's usage windows come from.
#[async_trait]
pub trait UsageSource: Send + Sync {
    /// The usage file's contents: `{"five_hour": {"utilization", "resets_at"}, "seven_day": {…}}`.
    async fn read(&self) -> Result<Value, Failure>;
}

/// Where the collector writes a line when a source fails or recovers.
pub type Log = Arc<dyn Fn(String) + Send + Sync>;
type Connect = Box<dyn Fn(&UsageCollector) -> Result<Arc<dyn UsageSource>, String> + Send + Sync>;

struct Tracked {
    spec: UsageCollector,
    source: Arc<dyn UsageSource>,
    due: Instant,
    failures: u32,
}

/// The collector's state: when each source is due and how often it failed in a row.
pub struct Collector {
    data_dir: PathBuf,
    connect: Connect,
    log: Log,
    tracked: HashMap<String, Tracked>,
}

impl Collector {
    pub fn new(data_dir: PathBuf, connect: Connect, log: Log) -> Self {
        Self {
            data_dir,
            connect,
            log,
            tracked: HashMap::new(),
        }
    }

    /// Reads every source with a collector that is due at `now`.
    pub async fn read_due(&mut self, sources: &[FundingConfig], now: Instant) {
        let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
        self.tracked.retain(|id, _| ids.contains(&id.as_str()));
        for source in sources {
            let Some(spec) = &source.collector else {
                continue;
            };
            let Some(path) = source.usage_path(&self.data_dir) else {
                continue;
            };
            if !self.track(source, spec, &path, now) {
                continue;
            }
            self.read_one(source.id.as_str(), &path, now).await;
        }
    }

    /// Whether the source is due. A source seen for the first time (or whose collector changed) waits
    /// out the rest of the interval since its file was written, so a node that restarts often does not
    /// ask often.
    fn track(
        &mut self,
        source: &FundingConfig,
        spec: &UsageCollector,
        path: &Path,
        now: Instant,
    ) -> bool {
        let id = source.id.as_str();
        if self
            .tracked
            .get(id)
            .is_none_or(|tracked| &tracked.spec != spec)
        {
            let connected = match (self.connect)(spec) {
                Ok(connected) => connected,
                Err(why) => {
                    (self.log)(format!("usage of \"{id}\" cannot be collected: {why}"));
                    self.tracked.remove(id);
                    return false;
                }
            };
            let due = now + INTERVAL.saturating_sub(file_age(path));
            self.tracked.insert(
                id.to_owned(),
                Tracked {
                    spec: spec.clone(),
                    source: connected,
                    due,
                    failures: 0,
                },
            );
        }
        self.tracked[id].due <= now
    }

    async fn read_one(&mut self, id: &str, path: &Path, now: Instant) {
        let source = self.tracked[id].source.clone();
        let outcome = source.read().await.and_then(|usage| {
            usage_file::write(path, &usage).map_err(|error| {
                Failure::Unavailable(format!("cannot write the usage file: {}", error.kind()))
            })
        });
        let tracked = self.tracked.get_mut(id).expect("tracked above");
        match outcome {
            Ok(()) => {
                if tracked.failures > 0 {
                    (self.log)(format!("usage of \"{id}\" is read again"));
                }
                tracked.failures = 0;
                tracked.due = now + INTERVAL;
            }
            Err(failure) => {
                tracked.failures += 1;
                let wait = wait_after_failure(tracked.failures, &failure);
                tracked.due = now + wait;
                (self.log)(format!(
                    "usage of \"{id}\" could not be read: {failure}; trying again in {} minutes",
                    wait.as_secs().div_ceil(60)
                ));
            }
        }
    }
}

fn file_age(path: &Path) -> Duration {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .unwrap_or(Duration::MAX)
}

/// The sources a node reads for real.
fn connect(clock: Arc<dyn Clock>, home: PathBuf, user: String) -> Connect {
    Box::new(move |spec| match spec {
        UsageCollector::Claude { config_dir } => {
            let store = ClaudeConfigDir::new(config_dir.clone(), home.clone(), user.clone());
            let usage = ClaudeUsage::new(Arc::new(store), clock.clone(), USAGE_URL.to_owned())
                .map_err(|error| error.without_url().to_string())?;
            Ok(Arc::new(usage) as Arc<dyn UsageSource>)
        }
        UsageCollector::Codex {
            codex_home,
            command,
        } => {
            let command = command.clone().unwrap_or_else(|| PathBuf::from("codex"));
            let usage = CodexUsage::new(command, codex_home.clone(), codex::ANSWER_WITHIN);
            Ok(Arc::new(usage) as Arc<dyn UsageSource>)
        }
    })
}

/// Reads the node's sources for as long as the node runs.
pub async fn run(node: Arc<Node>, data_dir: PathBuf, clock: Arc<dyn Clock>, log: Log) {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let user = std::env::var("USER").unwrap_or_default();
    let mut collector = Collector::new(data_dir, connect(clock, home, user), log);
    loop {
        collector
            .read_due(&node.config().funding, Instant::now())
            .await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

#[cfg(test)]
mod tests;
