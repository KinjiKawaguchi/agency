//! Signed reads (kind `read`): the reader signs the method, path, time, and account; the reading node
//! checks the signature against the org definition and answers only members of the org.
use crate::error::Result;
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
    let date = clock::format(now);
    let signature = key.sign(&Kind::Read.message(&read_text(method, path, &date, account)));
    ReadAuth {
        account: account.to_string(),
        date,
        signature,
    }
}

/// Checks a signed read against the definition of the org it reads (none when the org or team is not
/// here) and returns the member who signed it. Every failure is the same `NotFound` (see `unseen`):
/// a malformed or stale signature, no definition, a non-member, or a wrong signature. Once the
/// signature is well formed and fresh, one signature check is spent whatever is missing.
pub fn verify_read<'a>(
    definition: Option<&'a OrgDefinition>,
    auth: &ReadAuth,
    method: &str,
    path: &str,
    now: OffsetDateTime,
) -> Result<&'a Person> {
    let account = Id::parse(&auth.account).map_err(|_| unseen::not_found())?;
    let at = clock::parse(&auth.date).ok_or_else(unseen::not_found)?;
    if (now - at).whole_seconds().abs() > SKEW_SECONDS {
        return Err(unseen::not_found());
    }
    let person = definition.and_then(|definition| definition.member(&account));
    let key: &PublicKey = match person {
        Some(person) => &person.public_key,
        None => unseen::stand_in_key(),
    };
    let message = Kind::Read.message(&read_text(method, path, &auth.date, &account));
    let is_valid = key.verify(&message, &auth.signature);
    person.filter(|_| is_valid).ok_or_else(unseen::not_found)
}
