//! Agency's protocol between nodes (docs/protocol.md).
//! Every piece of data has one writer. The writer signs it; other nodes keep a replica only after checking the signature.

pub mod about;
pub mod activity;
pub mod budget;
pub mod canonical;
pub mod client;
pub mod client_version;
pub mod clock;
pub mod device;
pub mod digest;
pub mod directory;
pub mod effort;
pub mod envelope;
pub mod evaluation;
pub mod history;
pub mod ids;
pub mod invite;
pub mod keys;
pub mod metrics;
pub mod observation;
pub mod org;
pub mod outcome;
pub mod participant_metrics;
pub mod reading;
pub mod root_change;
pub mod runtime;
pub mod seat;
pub mod signed;
pub mod telemetry;
pub mod timestamp;
pub mod usage;
pub mod view;

pub use device::{DeviceChange, DeviceOp};
pub use envelope::{Envelope, EnvelopeKind, Payload};
pub use ids::{EnvelopeId, Id};
pub use invite::{Invite, JoinRequest};
pub use keys::{AccountKey, PublicKey};
pub use org::{Device, OrgDefinition, Person, Role};
pub use runtime::{RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse, RUNTIME_VERSION};
pub use signed::{Kind, Signed};
pub use timestamp::{Date, Timestamp};
pub use usage::TokenUsage;
pub use view::TeamView;
