//! Where a claim link stands for the person who opened it (docs/protocol.md, "Claim link"): what the node answers
//! to the browser that redeemed the link, so that it can show the pairing code and wait for the owner.
use crate::ids::Id;
use crate::timestamp::Timestamp;
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

/// The id a client names a waiting claim by: the org's id, a dot, and the seat's id (neither holds a dot).
pub fn claim_id(org: &Id, seat: &Id) -> String {
    format!("{org}.{seat}")
}

/// The org and the seat a claim id names, or `None` for a text that is not one.
pub fn parse_claim_id(raw: &str) -> Option<(Id, Id)> {
    let (org, seat) = raw.split_once('.')?;
    Some((Id::parse(org).ok()?, Id::parse(seat).ok()?))
}

/// A claim that waits for the owner (`GET /api/claims`): what the owner compares with what the person's screen
/// shows. It holds no address of the person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WaitingClaim {
    /// What the routes under `/api/claims/<id>/` name it by (`claim_id`).
    pub id: String,
    pub org: Id,
    pub seat: Id,
    /// The seat's label as the org's definition has it (its id when the seat has gone).
    pub seat_label: String,
    /// What the browser calls itself.
    pub device_label: String,
    /// The pairing code of the key the browser sent, for the owner to compare with the person's screen.
    pub pairing_code: String,
    pub redeemed_at: Timestamp,
}

/// The claims that wait for the owner, the oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaimList {
    pub claims: Vec<WaitingClaim>,
}

/// What the owner sends to confirm a claim: the pairing code read off the person's screen. An unknown field is
/// refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ClaimConfirmInput {
    pub pairing_code: String,
}

/// The answer to a confirm: the seat is now a member the node hosts, in this version of the org's definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaimConfirmed {
    pub org: Id,
    pub seat: Id,
    #[ts(type = "number")]
    pub version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_id_is_the_org_a_dot_and_the_seat_and_nothing_else_parses() {
        let (org, seat) = (Id::parse("acme").unwrap(), Id::parse("carol").unwrap());
        assert_eq!(claim_id(&org, &seat), "acme.carol");
        assert_eq!(parse_claim_id("acme.carol"), Some((org, seat)));
        for bad in [
            "",
            "acme",
            "acme.",
            ".carol",
            "acme.carol.x",
            "ACME.carol",
            "acme.car ol",
            "acme/carol",
        ] {
            assert_eq!(parse_claim_id(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_confirmation_refuses_a_field_it_does_not_know() {
        let ok: Result<ClaimConfirmInput, _> =
            serde_json::from_str(r#"{"pairingCode":"abcd-ef01-2345"}"#);
        assert_eq!(ok.unwrap().pairing_code, "abcd-ef01-2345");
        let extra: Result<ClaimConfirmInput, _> =
            serde_json::from_str(r#"{"pairingCode":"x","seat":"carol"}"#);
        assert!(extra.is_err());
    }
}
