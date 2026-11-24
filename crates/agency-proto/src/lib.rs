//! Agency の台どうしの約束 (docs/protocol.md)。
//! どのデータも書き手は 1 つ。書き手は署名し、ほかの台は署名を確かめてから写しを持つ。

pub mod canonical;
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
