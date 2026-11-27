//! Push registrations follow the devices of whoever registered them (docs/protocol.md, "Push notifications"):
//! the account's for the owner's, and the member's in its org for a guest's. A registration may name the
//! device it belongs to; it is accepted only for a current device of its own, and it is dropped when a
//! definition no longer has that device, so that a removed or lost device gets no more notices.
use crate::devices::{current_device_ids, my_devices};
use crate::error::{Error, Result};
use crate::node::Node;
use crate::push::remove_tokens;
use crate::web_push::remove_subscriptions;
use agency_proto::client::GuestRef;
use agency_proto::org::SignedOrg;
use agency_proto::Id;
use std::collections::BTreeMap;

/// The current devices of whoever a registration is of: the account's (`None`), or a guest's, which are the
/// member's in the guest's org.
fn device_ids_of(node: &Node, whose: Option<&GuestRef>) -> Vec<Id> {
    let Some(guest) = whose else {
        return current_device_ids(node);
    };
    node.store()
        .load_org(&guest.org)
        .map_or_else(Vec::new, |doc| {
            my_devices(&doc.body, &guest.member)
                .iter()
                .map(|device| device.id.clone())
                .collect()
        })
}

/// The device a registration names, if it is one of the current devices of whoever registers it.
pub(crate) fn check_device(
    node: &Node,
    whose: Option<&GuestRef>,
    device: Option<Id>,
) -> Result<Option<Id>> {
    let Some(device) = device else {
        return Ok(None);
    };
    if device_ids_of(node, whose).contains(&device) {
        return Ok(Some(device));
    }
    Err(Error::bad_request(
        "the device is not one of this account's devices",
    ))
}

/// Keeps a definition and drops the push registrations of every device it no longer has. Every definition
/// this node writes or learns is kept through here.
pub(crate) fn keep_org(node: &Node, doc: &SignedOrg) -> Result<()> {
    node.store().save_org(doc)?;
    drop_removed_devices(node)
}

/// What was dropped for one device: how many APNs tokens and how many browsers' subscriptions.
#[derive(Default)]
struct Dropped {
    tokens: Vec<String>,
    subscriptions: Vec<String>,
}

fn drop_removed_devices(node: &Node) -> Result<()> {
    // The devices of each owner of a registration, read once: the account's, and each guest's.
    let mut current: Vec<(Option<GuestRef>, Vec<Id>)> = Vec::new();
    let mut removed = |whose: Option<GuestRef>, device: Option<Id>| {
        let device = device?;
        if !current.iter().any(|(one, _)| *one == whose) {
            let ids = device_ids_of(node, whose.as_ref());
            current.push((whose.clone(), ids));
        }
        let is_current = current
            .iter()
            .any(|(one, ids)| *one == whose && ids.contains(&device));
        (!is_current).then_some(device)
    };
    let mut dropped: BTreeMap<Id, Dropped> = BTreeMap::new();
    for one in node.store().push_tokens() {
        if let Some(device) = removed(one.guest, one.device) {
            dropped.entry(device).or_default().tokens.push(one.token);
        }
    }
    for one in node.store().web_subscriptions() {
        if let Some(device) = removed(one.guest, one.device) {
            dropped
                .entry(device)
                .or_default()
                .subscriptions
                .push(one.id);
        }
    }
    for (device, gone) in &dropped {
        remove_tokens(node, &gone.tokens)?;
        remove_subscriptions(node, &gone.subscriptions)?;
        node.journal().info(
            "push",
            &format!(
                "dropped the push registrations of device {device}, which the account no longer has (push tokens: {}, browser subscriptions: {})",
                gone.tokens.len(),
                gone.subscriptions.len()
            ),
        );
    }
    Ok(())
}
