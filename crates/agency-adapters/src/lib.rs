//! Adapters for agency-core's ports.

/// Sent with every request this node makes. Some proxies (such as Cloudflare's Browser Integrity Check)
/// refuse requests without one.
pub const USER_AGENT: &str = concat!("agency-node/", env!("CARGO_PKG_VERSION"));

pub mod access;
pub mod apns;
pub mod client_api;
pub mod files;
pub mod funding;
pub mod github;
pub mod handoff;
pub mod launch;
pub mod memory;
pub mod peers;
pub mod refusal_log;
pub mod runtime;
pub mod server;
pub mod system;
pub mod usage_collector;

/// Where an adapter writes lines for the node's log.
pub type Log = Box<dyn Fn(&str) + Send + Sync>;
