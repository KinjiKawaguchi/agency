//! Claim links for seats. The secret code is returned once and is stored only as a SHA-256 digest.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{is_root, load};
use agency_proto::clock::{self, Duration};
use agency_proto::invite::{code_digest, code_from_bytes};
use agency_proto::Id;
use serde::{Deserialize, Serialize};

pub const CLAIM_DAYS: i64 = 7;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRecord {
    pub org: Id,
    pub seat: Id,
    pub email: String,
    pub expires_at: String,
    pub state: String,
}

#[derive(Debug)]
pub struct SeatInvite {
    pub link: String,
    pub email: String,
    pub local_only: bool,
}

/// Makes the one open claim link for `seat`, replacing any earlier one for it.
pub fn seat_invite(node: &Node, org: &Id, seat: Id, email: &str) -> Result<SeatInvite> {
    let current = load(node, org)?;
    if !is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("only the node with the root key of org {org} can make a seat claim link"),
        ));
    }
    if !current.body.seats.iter().any(|one| one.id == seat) {
        return Err(Error::not_found(format!("no seat {seat} in org {org}")));
    }
    let email = checked_email(email)?;
    if node.store().guest(&email).is_some() {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that email is already a guest",
        ));
    }
    let code = code_from_bytes(node.random());
    let digest = code_digest(&code);
    if let Some(old) = node.store().claim_for_seat(org, &seat) {
        node.store().remove_claim(&old)?;
    }
    let record = ClaimRecord {
        org: org.clone(),
        seat: seat.clone(),
        email: email.clone(),
        expires_at: clock::format(node.now() + Duration::days(CLAIM_DAYS)),
        state: "open".to_owned(),
    };
    node.store().save_claim(&digest, &record)?;
    node.store().save_claim_for_seat(org, &seat, &digest)?;
    let (base, local_only) = match node.config().access {
        Some(access) => (format!("https://{}", access.public_host), false),
        None => (node.url(), true),
    };
    Ok(SeatInvite {
        link: format!("{base}/claim#code={code}"),
        email,
        local_only,
    })
}

fn checked_email(raw: &str) -> Result<String> {
    let email = raw.to_lowercase();
    let parts: Vec<_> = email.split('@').collect();
    if email.chars().count() > 254
        || email.chars().any(char::is_control)
        || parts.len() != 2
        || parts.iter().any(|part| part.is_empty())
    {
        return Err(Error::bad_request("--email must have one @, text on both sides, at most 254 characters, and no control character"));
    }
    Ok(email)
}
