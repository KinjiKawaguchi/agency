//! Notices for guests (docs/protocol.md, "Guests"): a guest's app and browsers are told, once, of each request
//! that starts waiting for its member in a team on this node, and of nothing else. The badge counts that
//! member's requests only. Titles follow `hideTitles` as the owner's do. A destination failing is not tried
//! again alone, as the owner's are: the notice is sent again on the next pass until one of the guest's
//! destinations takes it.
use crate::error::Result;
use crate::node::Node;
use crate::push::{
    is_hiding_titles, send_to_all, waiting_in_hosted, Candidate, Pass, NOTIFIED_KEPT,
};
use agency_proto::client::GuestRef;

/// The guests that have a destination on this node: each once.
fn with_destinations(node: &Node) -> Vec<GuestRef> {
    let tokens = node
        .store()
        .push_tokens()
        .into_iter()
        .filter_map(|one| one.guest);
    let browsers = node
        .store()
        .web_subscriptions()
        .into_iter()
        .filter_map(|one| one.guest);
    let mut all: Vec<GuestRef> = vec![];
    for guest in tokens.chain(browsers) {
        if !all.contains(&guest) {
            all.push(guest);
        }
    }
    all
}

/// The requests that wait for the guest's member in the teams on this node, while it is still a guest
/// (this node is the org's root and the member hosted); none otherwise.
async fn waiting_for(node: &Node, guest: &GuestRef, is_hidden: bool) -> Option<Vec<Candidate>> {
    let doc = node.store().load_org(&guest.org)?;
    let is_guest = crate::orgs::is_root(node, &doc)
        && doc
            .body
            .member(&guest.member)
            .is_some_and(|member| member.is_hosted());
    if !is_guest {
        return None;
    }
    let mut all = vec![];
    for place in &doc.body.teams {
        if let Some(hosted) = node.hosted(&guest.org, &place.id) {
            all.extend(waiting_in_hosted(node, &doc.body, &hosted, &guest.member, is_hidden).await);
        }
    }
    Some(all)
}

/// Tells one guest of the requests that started waiting for its member since its last pass. Its first pass
/// only records what is there, so nothing old is sent.
async fn notify_guest(
    node: &Node,
    guest: &GuestRef,
    is_hidden: bool,
    pass: &mut Pass,
) -> Result<()> {
    let Some(requests) = waiting_for(node, guest, is_hidden).await else {
        return Ok(());
    };
    let Some(mut notified) = node.store().guest_notified(guest) else {
        let keys: Vec<String> = requests.into_iter().map(|one| one.key).collect();
        return node.store().save_guest_notified(guest, &keys);
    };
    let badge = u32::try_from(requests.len()).unwrap_or(u32::MAX);
    let fresh: Vec<Candidate> = requests
        .into_iter()
        .filter(|one| !notified.contains(&one.key))
        .collect();
    for one in fresh {
        let notice = crate::push::Notice {
            badge,
            ..one.notice
        };
        let delivery = send_to_all(node, &notice, Some(guest), pass).await?;
        if delivery.is_done {
            pass.sent += 1;
            notified.push(one.key);
        }
    }
    let start = notified.len().saturating_sub(NOTIFIED_KEPT);
    node.store().save_guest_notified(guest, &notified[start..])
}

/// Tells every guest with a destination here of its new requests.
pub(crate) async fn notify_guests(node: &Node, pass: &mut Pass) -> Result<()> {
    let is_hidden = is_hiding_titles(node);
    for guest in with_destinations(node) {
        notify_guest(node, &guest, is_hidden, pass).await?;
    }
    Ok(())
}
