//! Adapters for agency-core's ports.

/// Sent with every request this node makes. Some proxies (such as Cloudflare's Browser Integrity Check)
/// refuse requests without one.
pub const USER_AGENT: &str = concat!("agency-node/", env!("CARGO_PKG_VERSION"));

pub mod access;
mod activity_window;
pub mod apns;
pub mod client_api;
pub mod files;
pub mod funding;
pub mod github;
pub mod handoff;
pub mod launch;
pub mod logging;
pub mod memory;
pub mod peers;
pub mod refusal_log;
pub mod runtime;
pub mod server;
mod swr;
pub use swr::ReadCounts;
pub mod system;
pub mod usage_collector;
pub mod web_push;

/// Where an adapter writes lines for the node's log.
pub type Log = Box<dyn Fn(&str) + Send + Sync>;
