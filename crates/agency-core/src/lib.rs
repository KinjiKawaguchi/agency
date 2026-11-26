//! Agency's domain and use cases. Everything outside the domain (time, randomness, storage, other nodes,
//! team runtimes) goes through the ports in `ports`, so this crate never touches the wall clock, the
//! network, or the filesystem. See docs/protocol.md.

pub mod about;
pub mod activity;
pub mod client;
pub mod config;
pub mod decisions;
pub mod delivery;
pub mod departments;
pub mod devices;
pub mod error;
pub mod evaluation;
pub mod funding_pick;
pub mod invites;
pub mod kpis;
pub mod logging_config;
pub mod node;
pub mod notice;
pub mod orgs;
pub mod outbox;
pub mod outcome_readings;
pub mod outcomes;
pub mod peer_about;
pub mod ports;
pub mod push;
mod push_events;
pub mod read_auth;
pub mod readings;
pub mod routes;
pub mod ships;
pub mod signed_decisions;
pub mod teams;
pub mod unseen;
pub mod usage;
pub mod views;
pub mod web_push;

pub use error::{Error, ErrorKind, Result};
pub use node::{Node, Ports};
