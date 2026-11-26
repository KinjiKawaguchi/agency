//! Web Push for the owner's browsers (docs/protocol.md, "Push notifications"). A browser registers the
//! subscription its push service gave it; the node sends it the same notices as an APNs token, encrypted
//! to the browser, through that push service. The node's VAPID key and the sending are the notifier's
//! (a port); this holds what the node decides: which endpoints it will ever send to, and the list.
use crate::error::{Error, Result};
use crate::node::Node;
use crate::notice::Notice;
use agency_proto::client::{
    PushEvent, PushSubscription, PushSubscriptionInput, PushSubscriptionList, VapidKey,
};
use agency_proto::digest::sha256_hex;
use agency_proto::Timestamp;
use serde::{Deserialize, Serialize};

/// The most subscriptions kept; the oldest is dropped first.
pub const MAX_SUBSCRIPTIONS: usize = 20;
const MAX_ENDPOINT: usize = 500;
/// Push services that are always allowed: a name, or `*.` and a domain.
const PUSH_SERVICES: [&str; 4] = [
    "fcm.googleapis.com",
    "updates.push.services.mozilla.com",
    "*.push.apple.com",
    "*.notify.windows.com",
];

/// A registered subscription as the node keeps it: with the endpoint and the keys it sends with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSubscription {
    pub id: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub label: String,
    pub registered_at: Timestamp,
    pub events: Vec<PushEvent>,
    pub key_id: String,
}

fn host_matches(pattern: &str, host: &str) -> bool {
    match pattern.strip_prefix("*.") {
        Some(domain) => host
            .strip_suffix(domain)
            .is_some_and(|front| front.len() > 1 && front.ends_with('.')),
        None => pattern.eq_ignore_ascii_case(host),
    }
}

/// An endpoint the node will send to: `https`, no credentials, no port, no fragment, a host name (never an
/// address) on the list of push services or the configured extras. Returns it with the host in lowercase.
/// The node does not follow redirects and checks the addresses again when it connects; this is the first
/// of the two checks, and the one that decides which services receive what the owner's teams ask.
pub fn check_endpoint(raw: &str, extra_hosts: &[String]) -> Result<String> {
    let refuse = || Error::bad_request("the endpoint is not one of an allowed push service");
    if raw.len() > MAX_ENDPOINT || raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(refuse());
    }
    let rest = raw.strip_prefix("https://").ok_or_else(refuse)?;
    let (host, path) = rest.split_once('/').ok_or_else(refuse)?;
    let is_name = host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && host.bytes().any(|b| b.is_ascii_alphabetic())
        && !host.starts_with('.')
        && !host.ends_with('.');
    if !is_name || path.contains('#') {
        return Err(refuse());
    }
    let host = host.to_ascii_lowercase();
    let is_allowed = PUSH_SERVICES
        .iter()
        .copied()
        .chain(extra_hosts.iter().map(String::as_str))
        .any(|pattern| host_matches(pattern, &host));
    if !is_allowed {
        return Err(refuse());
    }
    Ok(format!("https://{host}/{path}"))
}

/// What a subscription's key looks like before it is used: base64url without padding, of this many octets.
fn is_key(text: &str, octets: usize) -> bool {
    let chars = (octets * 4).div_ceil(3);
    text.len() == chars
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn subscription_id(endpoint: &str) -> String {
    sha256_hex(endpoint.as_bytes())[..16].to_owned()
}

/// The endpoint as a list shows it: its host and the end of its path, which is what tells two apart.
fn shortened(endpoint: &str) -> String {
    let rest = endpoint.strip_prefix("https://").unwrap_or(endpoint);
    let host = rest.split('/').next().unwrap_or(rest);
    let tail: Vec<char> = rest.chars().rev().take(8).collect();
    format!("https://{host}/…{}", tail.iter().rev().collect::<String>())
}

fn view(one: WebSubscription) -> PushSubscription {
    PushSubscription {
        endpoint: shortened(&one.endpoint),
        id: one.id,
        label: one.label,
        registered_at: one.registered_at,
        events: one.events,
        key_id: one.key_id,
    }
}

fn extra_hosts(node: &Node) -> Vec<String> {
    node.config()
        .web_push
        .map(|web| web.extra_hosts)
        .unwrap_or_default()
}

/// The node's VAPID key, or nothing when Web Push is not set up (the routes then answer 404).
pub fn vapid_key(node: &Node) -> Option<VapidKey> {
    node.notifier().web_push_key()
}

fn not_set_up() -> Error {
    Error::not_found("this node has no Web Push")
}

/// Registers a browser's subscription, or updates the one of the same endpoint (its label, keys, kinds).
pub fn register_subscription(
    node: &Node,
    input: PushSubscriptionInput,
) -> Result<PushSubscription> {
    let key = vapid_key(node).ok_or_else(not_set_up)?;
    let endpoint = check_endpoint(&input.endpoint, &extra_hosts(node))?;
    if !is_key(&input.p256dh, 65) || !is_key(&input.auth, 16) {
        return Err(Error::bad_request(
            "p256dh is 65 octets and auth is 16, as base64url",
        ));
    }
    let label = input.label.trim().to_owned();
    if !(1..=80).contains(&label.chars().count()) || label.chars().any(char::is_control) {
        return Err(Error::bad_request("the label is 1 to 80 characters"));
    }
    let registered_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let entry = WebSubscription {
        id: subscription_id(&endpoint),
        endpoint,
        p256dh: input.p256dh,
        auth: input.auth,
        label,
        registered_at,
        events: crate::push::normal_events(input.events),
        key_id: key.key_id,
    };
    let mut all: Vec<WebSubscription> = node
        .store()
        .web_subscriptions()
        .into_iter()
        .filter(|one| one.id != entry.id)
        .collect();
    all.push(entry.clone());
    let start = all.len().saturating_sub(MAX_SUBSCRIPTIONS);
    node.store().save_web_subscriptions(&all[start..])?;
    Ok(view(entry))
}

/// Removes a subscription by its id. An unknown id is not an error.
pub fn remove_subscription(node: &Node, id: &str) -> Result<()> {
    remove_subscriptions(node, &[id.to_owned()])
}

pub(crate) fn remove_subscriptions(node: &Node, gone: &[String]) -> Result<()> {
    let kept: Vec<WebSubscription> = node
        .store()
        .web_subscriptions()
        .into_iter()
        .filter(|one| !gone.contains(&one.id))
        .collect();
    node.store().save_web_subscriptions(&kept)
}

pub fn subscriptions(node: &Node) -> Result<PushSubscriptionList> {
    vapid_key(node).ok_or_else(not_set_up)?;
    Ok(PushSubscriptionList {
        subscriptions: node
            .store()
            .web_subscriptions()
            .into_iter()
            .map(view)
            .collect(),
    })
}

/// The subscriptions that get this notice: of its kind, and made with the node's current key (one made
/// with another key is refused by the push service, and the screen tells the owner to subscribe again).
pub(crate) fn wanting(node: &Node, notice: &Notice) -> Vec<WebSubscription> {
    let Some(key) = vapid_key(node) else {
        return vec![];
    };
    node.store()
        .web_subscriptions()
        .into_iter()
        .filter(|one| one.key_id == key.key_id && one.events.contains(&notice.event()))
        .collect()
}
