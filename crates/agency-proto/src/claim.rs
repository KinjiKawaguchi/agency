//! Where a claim link stands for the person who opened it (docs/protocol.md, "Claim link"): what the node answers
//! to the browser that redeemed the link, so that it can show the pairing code and wait for the owner.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a browser sends to redeem a claim link: the code of the link, the device key it just made (as text, Ed25519
/// or P-256), and what it calls itself. An unknown field is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ClaimInput {
    pub code: String,
    pub public_key: String,
    pub label: String,
}

/// Whether the owner has confirmed the claim yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ClaimProgress {
    /// The browser's key is kept, and the owner has not compared the pairing code and confirmed.
    Waiting,
    /// The owner confirmed: the person is a member the node hosts.
    In,
}

/// The answer to a redeem of a claim link, and to a later ask for where it stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaimStatus {
    pub state: ClaimProgress,
    /// The seat's label, as the org writes it.
    pub seat: String,
    /// The org's label.
    pub org: String,
    /// The pairing code of the key the browser sent (the first 12 hex characters of its fingerprint, in groups of
    /// four), for the owner to compare with the one this browser shows.
    pub pairing_code: String,
}
