//! Push registrations follow the account's devices (docs/protocol.md, "Push notifications"). A registration
//! may name the device it belongs to; it is accepted only for a current device, and it is dropped when a
//! definition no longer has that device, so that a removed or lost device gets no more notices.
use crate::devices::current_device_ids;
use crate::error::{Error, Result};
use crate::node::Node;
use crate::push::remove_tokens;
use crate::web_push::remove_subscriptions;
use agency_proto::org::SignedOrg;
use agency_proto::Id;
use std::collections::BTreeMap;

/// The device a registration names, if it is one of the account's current devices.
pub(crate) fn check_device(node: &Node, device: Option<Id>) -> Result<Option<Id>> {
    let Some(device) = device else {
        return Ok(None);
    };
    if current_device_ids(node).contains(&device) {
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
    crate::read_auth::drop_departed_readers(node);
    drop_removed_devices(node)
}

/// What was dropped for one device: how many APNs tokens and how many browsers' subscriptions.
#[derive(Default)]
struct Dropped {
    tokens: Vec<String>,
    subscriptions: Vec<String>,
}

fn drop_removed_devices(node: &Node) -> Result<()> {
    let current = current_device_ids(node);
    let removed = |device: Option<Id>| device.filter(|id| !current.contains(id));
    let mut dropped: BTreeMap<Id, Dropped> = BTreeMap::new();
    for one in node.store().push_tokens() {
        if let Some(device) = removed(one.device) {
            dropped.entry(device).or_default().tokens.push(one.token);
        }
    }
    for one in node.store().web_subscriptions() {
        if let Some(device) = removed(one.device) {
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
