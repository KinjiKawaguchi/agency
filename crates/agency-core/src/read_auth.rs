//! Signed reads (kind `read`): the reader signs the method, path, time, and account; the reading node
//! checks the signature against the org definition and answers only members of the org.
use crate::error::{Error, Result};
use agency_proto::clock::{self, OffsetDateTime};
use agency_proto::org::OrgDefinition;
use agency_proto::signed::read_text;
use agency_proto::{AccountKey, Id, Kind, Person};

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
    let date = clock::format(now);
    let signature = key.sign(&Kind::Read.message(&read_text(method, path, &date, account)));
    ReadAuth {
        account: account.to_string(),
        date,
        signature,
    }
}

/// Checks a signed read and returns the member who signed it.
/// A non-member gets `NotFound`, so a stranger cannot tell whether the org or team exists.
pub fn verify_read<'a>(
    definition: &'a OrgDefinition,
    auth: &ReadAuth,
    method: &str,
    path: &str,
    now: OffsetDateTime,
) -> Result<&'a Person> {
    let account = Id::parse(&auth.account)
        .map_err(|_| Error::unauthorized("missing or malformed account"))?;
    let at =
        clock::parse(&auth.date).ok_or_else(|| Error::unauthorized("missing or malformed date"))?;
    if (now - at).whole_seconds().abs() > SKEW_SECONDS {
        return Err(Error::unauthorized(
            "the signed time is too old or too far ahead",
        ));
    }
    let person = definition
        .member(&account)
        .ok_or_else(|| Error::not_found("not found"))?;
    let message = Kind::Read.message(&read_text(method, path, &auth.date, &account));
    if !person.public_key.verify(&message, &auth.signature) {
        return Err(Error::unauthorized("the signature does not match"));
    }
    Ok(person)
}
