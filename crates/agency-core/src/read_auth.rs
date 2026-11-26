//! Signed reads (kind `read`): the reader signs the method, path, time, and account; the reading node
//! checks the signature against the org definition and answers only members of the org.
use crate::error::Result;
use crate::node::Node;
use crate::unseen;
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::org::OrgDefinition;
use agency_proto::signed::read_text;
use agency_proto::{AccountKey, Id, Kind, Person, PublicKey};

/// How far a signed time may be from the reading node's clock, either way. It also limits how long
/// a captured signature can be reused.
pub const SKEW_SECONDS: i64 = 300;

/// The read signature as it travels: account, time, and signature, unchecked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadAuth {
    pub account: String,
    pub date: String,
    pub signature: String,
}

pub fn sign_read(
    key: &AccountKey,
    account: &Id,
    method: &str,
    path: &str,
    now: OffsetDateTime,
) -> ReadAuth {
    let date = clock::format_millis(now);
    let signature = key.sign(&Kind::Read.message(&read_text(method, path, &date, account)));
    ReadAuth {
        account: account.to_string(),
        date,
        signature,
    }
}

/// Checks a signed read and returns the member who signed it. `definition` is the org's, if this node
/// has it; `is_here` says whether what is read is on this node (the team runs here, or, for the
/// definition itself, the org is here). Every failure is the same `NotFound` (see `unseen`): a
/// malformed or stale signature, no definition, a non-member, a wrong signature, or nothing here. Once
/// the account and time are well formed, one signature check is spent whatever is missing.
pub fn verify_read<'a>(
    definition: Option<&'a OrgDefinition>,
    is_here: bool,
    auth: &ReadAuth,
    method: &str,
    path: &str,
    now: OffsetDateTime,
) -> Result<&'a Person> {
    let account = Id::parse(&auth.account)
        .map_err(|_| unseen::refused("a signed read without a well-formed account"))?;
    let at = clock::parse(&auth.date).ok_or_else(|| {
        unseen::refused(format!(
            "a signed read by {account} without a well-formed time"
        ))
    })?;
    let person = definition.and_then(|definition| definition.member(&account));
    let key: &PublicKey = match person {
        Some(person) => &person.public_key,
        None => unseen::stand_in_key(),
    };
    let message = Kind::Read.message(&read_text(method, path, &auth.date, &account));
    let is_valid = key.verify(&message, &auth.signature);
    let skew = (now - at).whole_seconds();
    match (definition, person) {
        (None, _) => Err(unseen::refused(format!(
            "a read by {account} in an org this node does not have"
        ))),
        (Some(definition), None) => Err(unseen::refused(format!(
            "a read by {account}, who is not a member of {}",
            definition.org
        ))),
        (Some(_), Some(_)) if !is_valid => Err(unseen::refused(format!(
            "a read by {account} whose signature does not match their key"
        ))),
        (Some(_), Some(_)) if skew.abs() > SKEW_SECONDS => Err(unseen::refused_member(format!(
            "a read signed by {account} at {}, {} seconds {} this node's clock; check the clocks",
            clock::format(at),
            skew.abs(),
            if skew > 0 { "behind" } else { "ahead of" }
        ))),
        (Some(definition), Some(_)) if !is_here => Err(unseen::refused_member(format!(
            "a read by {account} of something in {} that is not on this node",
            definition.org
        ))),
        (Some(_), Some(person)) => Ok(person),
    }
}

/// `verify_read`, accepted once per signature (`Node::accept_read_once`): a copy of a valid read sent
/// again is refused like any failed read.
pub fn verify_read_once<'a>(
    node: &Node,
    definition: Option<&'a OrgDefinition>,
    is_here: bool,
    auth: &ReadAuth,
    method: &str,
    path: &str,
) -> Result<&'a Person> {
    let person = verify_read(definition, is_here, auth, method, path, node.now())?;
    let signed_at = clock::parse(&auth.date)
        .ok_or_else(|| unseen::refused("a signed read without a well-formed time"))?;
    if !node.accept_read_once(&auth.signature, signed_at) {
        // Anyone who saw the read can send the copy, so it is not vouched for by the member.
        return Err(unseen::refused(format!(
            "a copy of a read by {} that was already accepted",
            person.id
        )));
    }
    Ok(person)
}
