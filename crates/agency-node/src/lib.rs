//! Agency のノード。機械ごとに 1 つ動き、この台のチームの様子を配り、署名した決裁を受け取る。
//! ほかの台のチームは写しを持ち、ほかの台への決裁は送り箱にため置いて送る。

pub mod auth;
pub mod client;
pub mod config;
pub mod runtime;
pub mod node;
pub mod orgs;
pub mod outbox;
pub mod server;
pub mod store;
