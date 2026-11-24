//! Agency's protocol between nodes (docs/protocol.md).
//! Every piece of data has one writer. The writer signs it; other nodes keep a replica only after checking the signature.

pub mod canonical;
pub mod client;
pub mod clock;
pub mod digest;
pub mod envelope;
pub mod ids;
pub mod invite;
pub mod keys;
pub mod org;
pub mod runtime;
pub mod signed;
pub mod timestamp;
pub mod view;

pub use envelope::{Envelope, EnvelopeKind, Payload};
pub use ids::{EnvelopeId, Id};
pub use invite::{Invite, JoinRequest};
pub use keys::{AccountKey, PublicKey};
pub use org::{OrgDefinition, Person, Role};
pub use runtime::{RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse, RUNTIME_VERSION};
pub use signed::{Kind, Signed};
pub use timestamp::{Date, Timestamp};
pub use view::TeamView;
