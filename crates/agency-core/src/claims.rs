//! Claim links for seats. The secret code is returned once and is stored only as a SHA-256 digest.
use crate::error::{Error, ErrorKind, Result};
use crate::guests::check_email;
use crate::node::Node;
use crate::orgs::{is_root, load};
use agency_proto::claim::{ClaimProgress, ClaimStatus};
use agency_proto::clock::{self, Duration};
use agency_proto::invite::{code_digest, code_from_bytes};
use agency_proto::org::{label_ok_to_write, Device, SignedOrg};
use agency_proto::{Id, PublicKey, Timestamp};
use serde::{Deserialize, Serialize};

pub const CLAIM_DAYS: i64 = 7;

/// A claim link. `Open` is written as the text `open`, as every record made before the link could be redeemed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRecord {
    pub org: Id,
    pub seat: Id,
    pub email: String,
    pub expires_at: String,
    pub state: ClaimState,
}

/// Where a claim link stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaimState {
    /// Not redeemed yet.
    Open,
    /// A browser redeemed it with its device key; the owner has not confirmed.
    Redeemed(Redemption),
    /// The owner confirmed, and the seat became a hosted member.
    Confirmed(Redemption),
}

/// What a browser sent when it redeemed a claim link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Redemption {
    /// The device key the browser made.
    pub public_key: PublicKey,
    /// What the browser calls itself (1 to 80 characters).
    pub label: String,
    pub redeemed_at: String,
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
    let email = check_email(email)?;
    if node
        .config()
        .access
        .as_ref()
        .is_some_and(|access| access.owner_email.trim().eq_ignore_ascii_case(&email))
    {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that email is the node owner's",
        ));
    }
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
        state: ClaimState::Open,
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

fn unknown() -> Error {
    Error::not_found("that link is not valid or was already used")
}

/// The record of an open link that `code` names, for `email`, and the digest it is kept under: everything that
/// refuses a redeem before the key and label are looked at. An unknown code and a spent one are the same answer.
fn open_claim(node: &Node, code: &str, email: &str) -> Result<(String, ClaimRecord)> {
    let digest = code_digest(code);
    let record = node.store().load_claim(&digest).ok_or_else(unknown)?;
    if record.state != ClaimState::Open {
        return Err(unknown());
    }
    let is_expired = clock::parse(&record.expires_at).is_none_or(|at| at <= node.now());
    if is_expired {
        return Err(Error::new(ErrorKind::Gone, "that link has expired"));
    }
    if record.email != email.trim().to_lowercase() {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "that link was made for another address",
        ));
    }
    Ok((digest, record))
}

/// A browser redeems a claim link: `email` is the address the person signed in with (Access has verified it),
/// `public_key` the device key the browser made, `label` what it calls itself. Refused with the same answer for a
/// code that is no code, unknown, or spent (404); 410 once expired; 403 for another address; 409 when the seat
/// has gone, or the key is one the org already has or another claim holds; 400 for a key that is not a device
/// key or a label that is not 1 to 80 characters without hidden ones. No answer holds an address, a code or a
/// key. The code is spent here, under the write lock, so no two redeems of one link both succeed.
pub async fn redeem(
    node: &Node,
    code: &str,
    email: &str,
    public_key: &str,
    label: &str,
) -> Result<ClaimStatus> {
    // Nobody waits for the lock for a link that is not open for this address.
    open_claim(node, code, email)?;
    let _guard = node.write_lock().lock().await;
    let (digest, mut record) = open_claim(node, code, email)?;
    let doc = load(node, &record.org).map_err(|_| unknown())?;
    let seat = doc
        .body
        .seats
        .iter()
        .find(|one| one.id == record.seat)
        .ok_or_else(|| Error::new(ErrorKind::Conflict, "that seat is not in the org any more"))?;
    let key = PublicKey::try_from(public_key.to_owned())
        .map_err(|_| Error::bad_request("the key is not an Ed25519 or P-256 device key"))?;
    if doc.body.has_key(&key) || is_held_by_a_claim(node, &key) {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that key is already in use in this org",
        ));
    }
    if !label_ok_to_write(label) {
        return Err(Error::bad_request(
            "the label must be 1 to 80 characters with nothing that hides or reorders text",
        ));
    }
    let status = ClaimStatus {
        state: ClaimProgress::Waiting,
        seat: seat.label.clone(),
        org: doc.body.label.clone(),
        pairing_code: key.pairing_code(),
    };
    record.state = ClaimState::Redeemed(Redemption {
        public_key: key,
        label: label.to_owned(),
        redeemed_at: node.now_text(),
    });
    node.store().save_claim(&digest, &record)?;
    Ok(status)
}

/// Whether a claim that was redeemed (and not rejected) already holds `key`.
fn is_held_by_a_claim(node: &Node, key: &PublicKey) -> bool {
    node.store()
        .claims()
        .iter()
        .any(|(_, record)| match &record.state {
            ClaimState::Redeemed(redemption) | ClaimState::Confirmed(redemption) => {
                &redemption.public_key == key
            }
            ClaimState::Open => false,
        })
}

/// Where the claim of the person who signed in with `email` stands: its seat's and org's labels, the pairing code
/// of the key the browser sent, and whether the owner has confirmed. When more than one seat was claimed with the
/// address, the one redeemed last. Not found when none was redeemed.
pub fn state(node: &Node, email: &str) -> Result<ClaimStatus> {
    let email = email.trim().to_lowercase();
    let mut mine: Vec<(ClaimRecord, Redemption, ClaimProgress)> = node
        .store()
        .claims()
        .into_iter()
        .filter(|(_, record)| record.email == email)
        .filter_map(|(_, record)| match record.state.clone() {
            ClaimState::Redeemed(one) => Some((record, one, ClaimProgress::Waiting)),
            ClaimState::Confirmed(one) => Some((record, one, ClaimProgress::In)),
            ClaimState::Open => None,
        })
        .collect();
    mine.sort_by(|a, b| a.1.redeemed_at.cmp(&b.1.redeemed_at));
    let (record, redemption, progress) = mine.pop().ok_or_else(unknown)?;
    let doc = load(node, &record.org).map_err(|_| unknown())?;
    let label = match progress {
        ClaimProgress::In => doc.body.member(&record.seat).map(|one| one.label.clone()),
        ClaimProgress::Waiting => doc
            .body
            .seats
            .iter()
            .find(|one| one.id == record.seat)
            .map(|one| one.label.clone()),
    };
    Ok(ClaimStatus {
        state: progress,
        seat: label.ok_or_else(unknown)?,
        org: doc.body.label.clone(),
        pairing_code: redemption.public_key.pairing_code(),
    })
}

/// Whether `email` has a claim link that can still be used or is waiting for the owner: one not yet redeemed and
/// not expired, or one redeemed and not confirmed. Only such an address is admitted, and only to the claim.
pub fn is_claimant(node: &Node, email: &str) -> bool {
    let email = email.trim().to_lowercase();
    let now = node.now();
    node.store()
        .claims()
        .iter()
        .filter(|(_, record)| record.email == email)
        .any(|(_, record)| match record.state {
            ClaimState::Open => clock::parse(&record.expires_at).is_some_and(|at| at > now),
            ClaimState::Redeemed(_) => true,
            ClaimState::Confirmed(_) => false,
        })
}

/// A claim a browser has redeemed and the owner has not confirmed yet, as the owner sees it: no address, no key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingClaim {
    pub org: Id,
    pub seat: Id,
    /// The seat's label as the definition has it (its id when the seat has gone).
    pub seat_label: String,
    /// What the browser calls itself.
    pub device_label: String,
    /// The pairing code of the key the browser sent, for the owner to compare with the one it shows the person.
    pub pairing_code: String,
    pub redeemed_at: String,
}

/// The claims waiting for the owner, the oldest first.
pub fn pending(node: &Node) -> Vec<PendingClaim> {
    let mut waiting: Vec<PendingClaim> = node
        .store()
        .claims()
        .into_iter()
        .filter_map(|(_, record)| {
            let ClaimState::Redeemed(redemption) = record.state else {
                return None;
            };
            let seat_label = load(node, &record.org)
                .ok()
                .and_then(|doc| {
                    doc.body
                        .seats
                        .iter()
                        .find(|one| one.id == record.seat)
                        .map(|one| one.label.clone())
                })
                .unwrap_or_else(|| record.seat.to_string());
            Some(PendingClaim {
                org: record.org,
                seat: record.seat,
                seat_label,
                device_label: redemption.label,
                pairing_code: redemption.public_key.pairing_code(),
                redeemed_at: redemption.redeemed_at,
            })
        })
        .collect();
    waiting.sort_by(|a, b| a.redeemed_at.cmp(&b.redeemed_at));
    waiting
}

/// The claim of `seat` in `org` in the state `want` picks, with the digest it is kept under.
fn claim_of(
    node: &Node,
    org: &Id,
    seat: &Id,
    want: impl Fn(&ClaimState) -> bool,
) -> Option<(String, ClaimRecord)> {
    node.store()
        .claims()
        .into_iter()
        .find(|(_, record)| &record.org == org && &record.seat == seat && want(&record.state))
}

/// What a pairing code is compared as: without dashes or spaces, in lower case.
fn plain(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// The owner confirms the claim of `seat`: `pairing` is the code the owner read off the person's screen, and it
/// must be the pairing code of the key the node holds (so a stale list, or a claim that was replaced, cannot be
/// confirmed). The node then makes the seat a member it hosts (`seats::host_seat`, which checks the seat and the key
/// again and keeps the person's address), and only once the root has signed is the claim confirmed. Confirming a
/// claim that is already confirmed answers as the first time did.
pub async fn confirm(node: &Node, org: &Id, seat: &Id, pairing: &str) -> Result<SignedOrg> {
    let _guard = node.write_lock().lock().await;
    let Some((digest, mut record)) = claim_of(node, org, seat, |state| {
        matches!(state, ClaimState::Redeemed(_))
    }) else {
        if claim_of(node, org, seat, |state| {
            matches!(state, ClaimState::Confirmed(_))
        })
        .is_some()
        {
            return load(node, org);
        }
        return Err(Error::not_found(format!(
            "no claim of seat {seat} waits in org {org}"
        )));
    };
    let ClaimState::Redeemed(redemption) = record.state.clone() else {
        return Err(unknown());
    };
    if plain(pairing) != plain(&redemption.public_key.pairing_code()) {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that is not the pairing code of the key this claim holds; compare it again on the person's screen",
        ));
    }
    let doc = load(node, org)?;
    let is_hosted_already = doc.body.member(seat).is_some_and(|member| {
        member.is_hosted()
            && member
                .devices
                .iter()
                .any(|device| device.public_key == redemption.public_key)
    });
    let doc = if is_hosted_already {
        doc
    } else {
        let device = Device {
            id: redemption.public_key.device_id(),
            label: redemption.label.clone(),
            public_key: redemption.public_key.clone(),
            added_at: Timestamp::parse(&node.now_text())
                .map_err(|error| Error::internal(error.to_string()))?,
        };
        crate::seats::host_seat(node, org, seat, device, &record.email)?
    };
    record.state = ClaimState::Confirmed(redemption);
    node.store().save_claim(&digest, &record)?;
    Ok(doc)
}

/// `confirm` for the owner's client: when the root key asks for the owner's presence it is the node's terminal
/// that is present, not the client, so the answer is a conflict that says to confirm there
/// (`agency claim-confirm`); holding the confirmation for the owner's presence is a change of its own.
pub async fn confirm_from_client(
    node: &Node,
    org: &Id,
    seat: &Id,
    pairing: &str,
) -> Result<SignedOrg> {
    let doc = load(node, org)?;
    if crate::pending_root::must_hold(node, &doc) {
        return Err(Error::new(
            ErrorKind::Conflict,
            "this node's root key asks for your presence: confirm at the node's terminal with `agency claim-confirm`",
        ));
    }
    confirm(node, org, seat, pairing).await
}

/// The owner refuses the claim of `seat`: the pending claim is dropped, and the link stays spent (a new link is
/// needed). Not found when none waits.
pub async fn reject(node: &Node, org: &Id, seat: &Id) -> Result<()> {
    let _guard = node.write_lock().lock().await;
    let (digest, _) = claim_of(node, org, seat, |state| {
        matches!(state, ClaimState::Redeemed(_))
    })
    .ok_or_else(|| Error::not_found(format!("no claim of seat {seat} waits in org {org}")))?;
    node.store().remove_claim(&digest)
}
