//! Agency's domain and use cases. Everything outside the domain (time, randomness, storage, other nodes,
//! team runtimes) goes through the ports in `ports`, so this crate never touches the wall clock, the
//! network, or the filesystem. See docs/protocol.md.

pub mod config;
pub mod decisions;
pub mod error;
pub mod invites;
pub mod node;
pub mod orgs;
pub mod outbox;
pub mod ports;
pub mod read_auth;
pub mod routes;
pub mod views;

pub use error::{Error, ErrorKind, Result};
pub use node::{Node, Ports};
