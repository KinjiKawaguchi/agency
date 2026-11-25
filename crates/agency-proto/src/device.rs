//! Device keys: the keys a person's clients hold, and the only keys that sign decisions. A device is added
//! or removed by a signed device change (kind `device`), which the org's root node writes into the next
//! org definition version.
use crate::envelope::{is_fresh_at, Envelope};
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::org::{Device, OrgDefinition, Person};
use crate::signed::{Kind, Signed};
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A change to one person's devices, signed by that person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceChange {
    pub org: Id,
    /// The person's account.
    pub account: Id,
    #[serde(flatten)]
    #[ts(flatten)]
    pub op: DeviceOp,
    pub at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "op", rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceOp {
    Add {
        device: Box<Device>,
    },
    /// Removes the device and revokes its key.
    Remove {
        device: Id,
    },
}

/// The key a device change must be signed with. The account key signs only while the person has no
/// devices, which leaves only adding the first one; after that, only their devices sign changes.
fn change_key<'a>(person: &'a Person, signer: &Id) -> Result<&'a PublicKey, String> {
    if person.devices.is_empty() {
        if signer == &person.account {
            return Ok(&person.public_key);
        }
        return Err(
            "a person without devices adds their first device with their account key".into(),
        );
    }
    person
        .devices
        .iter()
        .find(|device| &device.id == signer)
        .map(|device| &device.public_key)
        .ok_or_else(|| "not signed by one of the person's devices".into())
}

/// Checks a signed device change against the current definition: its org and person, who signed it,
/// that it is fresh, and that the result is a well-formed definition. Returns the changed definition's
/// contents; the root node makes it the next version.
pub fn check_device_change(
    current: &OrgDefinition,
    signed: &Signed<DeviceChange>,
    now: time::OffsetDateTime,
) -> Result<OrgDefinition, String> {
    let change = &signed.body;
    if change.org != current.org {
        return Err("the device change is for another org".into());
    }
    let person = current
        .member(&change.account)
        .ok_or("the account is not a member of the org")?;
    let key = change_key(person, &signed.signer)?;
    if !signed.verify(Kind::Device, key) {
        return Err("the signature of the device change does not match".into());
    }
    if !is_fresh_at(change.at.as_str(), now) {
        return Err("the device change is too old or too far ahead".into());
    }
    let changed = apply(current, change)?;
    changed.validate()?;
    Ok(changed)
}

/// The definition with the change applied. A removed device's key is revoked, so it can never sign again.
fn apply(current: &OrgDefinition, change: &DeviceChange) -> Result<OrgDefinition, String> {
    let mut next = current.clone();
    let person = next
        .people
        .iter_mut()
        .find(|person| person.account == change.account)
        .ok_or("the account is not a member of the org")?;
    match &change.op {
        DeviceOp::Add { device } => person.devices.push(Device::clone(device)),
        DeviceOp::Remove { device } => {
            let index = person
                .devices
                .iter()
                .position(|one| &one.id == device)
                .ok_or_else(|| format!("no device {device}"))?;
            let removed = person.devices.remove(index);
            next.revoked.push(removed.public_key);
        }
    }
    Ok(next)
}

/// The member who decided: the envelope's account, signed with one of that member's devices (named by
/// `signer`). An account key never signs a decision.
pub fn envelope_signer<'a>(
    definition: &'a OrgDefinition,
    signed: &Signed<Envelope>,
) -> Result<&'a Person, String> {
    let (person, device) = definition
        .device(&signed.body.account, &signed.signer)
        .ok_or("not signed by a device of a member of the org")?;
    if !signed.verify(Kind::Envelope, &device.public_key) {
        return Err("the signature of the envelope does not match".into());
    }
    Ok(person)
}
