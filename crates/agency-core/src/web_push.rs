//! Web Push for the owner's browsers (docs/protocol.md, "Push notifications"). A browser registers the
//! subscription its push service gave it; the node sends it the same notices as an APNs token, encrypted
//! to the browser, through that push service. The node's VAPID key and the sending are the notifier's
//! (a port); this holds what the node decides: which endpoints it will ever send to, and the list.
use crate::error::{Error, Result};
use crate::node::Node;
use crate::notice::Notice;
use agency_proto::client::{
    PushEvent, PushFailure, PushSubscription, PushSubscriptionInput, PushSubscriptionList, VapidKey,
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
    /// When a push service last took a notice for this browser, and the last time it did not.
    #[serde(default)]
    pub last_succeeded_at: Option<Timestamp>,
    #[serde(default)]
    pub last_failure: Option<PushFailure>,
    /// The guest this browser is a guest's, by org and member; absent for the owner's browsers. A guest's
    /// browser gets only its own requests' notices, and the owner's never a guest's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guest: Option<agency_proto::client::GuestRef>,
}

/// A DNS host name: at least two labels of letters, digits, and hyphens, the last starting with a letter. That
/// leaves out every form a parser may read as an IPv4 address (`127.0.0.1`, `0x7f.1`, `2130706433`, `0177.0.0.1`).
pub fn is_host_name(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let is_label = |label: &&str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(is_label)
        && labels
            .last()
            .is_some_and(|last| last.starts_with(|c: char| c.is_ascii_alphabetic()))
}

/// An entry of the list of push services: a host name in lowercase, or `*.` and a domain of at least two labels.
pub fn is_host_pattern(entry: &str) -> bool {
    is_host_name(entry.strip_prefix("*.").unwrap_or(entry)) && entry == entry.to_ascii_lowercase()
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
    let host = host.to_ascii_lowercase();
    if !is_host_name(&host) || path.contains('#') {
        return Err(refuse());
    }
    let is_allowed = PUSH_SERVICES
        .iter()
        .copied()
        .chain(
            extra_hosts
                .iter()
                .map(String::as_str)
                .filter(|entry| is_host_pattern(entry)),
        )
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
        last_succeeded_at: one.last_succeeded_at,
        last_failure: one.last_failure,
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
    register_subscription_as(node, &crate::guests::Caller::Owner, input)
}

/// `register_subscription` for the caller: a guest's browser is kept as the guest's, and never takes over a
/// browser kept as someone else's (the owner's, or another guest's), nor the other way round.
pub fn register_subscription_as(
    node: &Node,
    caller: &crate::guests::Caller,
    input: PushSubscriptionInput,
) -> Result<PushSubscription> {
    let key = vapid_key(node).ok_or_else(not_set_up)?;
    let endpoint = check_endpoint(&input.endpoint, &extra_hosts(node))?;
    if !is_key(&input.p256dh, 65) || !is_key(&input.auth, 16) {
        return Err(Error::bad_request(
            "p256dh is 65 octets and auth is 16, as base64url",
        ));
    }
    node.notifier()
        .web_push_check_keys(&input.p256dh, &input.auth)
        .map_err(Error::bad_request)?;
    let label = input.label.trim().to_owned();
    if !(1..=80).contains(&label.chars().count()) || label.chars().any(char::is_control) {
        return Err(Error::bad_request("the label is 1 to 80 characters"));
    }
    let registered_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let id = subscription_id(&endpoint);
    let known = node
        .store()
        .web_subscriptions()
        .into_iter()
        .find(|one| one.id == id);
    let guest = caller.guest_of();
    if known.as_ref().is_some_and(|one| one.guest != guest) {
        return Err(Error::new(
            crate::error::ErrorKind::Conflict,
            "this browser is registered for someone else on this node",
        ));
    }
    let entry = WebSubscription {
        id,
        endpoint,
        p256dh: input.p256dh,
        auth: input.auth,
        label,
        registered_at,
        events: caller.events(crate::push::normal_events(input.events)),
        key_id: key.key_id,
        // What was last known of this browser's push service is kept when the browser registers again.
        last_succeeded_at: known.as_ref().and_then(|one| one.last_succeeded_at.clone()),
        last_failure: known.and_then(|one| one.last_failure),
        guest,
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
    remove_subscription_as(node, &crate::guests::Caller::Owner, id)
}

/// Removes one of the caller's own subscriptions; another's, like an unknown id, is left as it is.
pub fn remove_subscription_as(node: &Node, caller: &crate::guests::Caller, id: &str) -> Result<()> {
    let guest = caller.guest_of();
    let is_own = node
        .store()
        .web_subscriptions()
        .iter()
        .any(|one| one.id == id && one.guest == guest);
    if !is_own {
        return Ok(());
    }
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
    subscriptions_of(node, &crate::guests::Caller::Owner)
}

/// The caller's own browsers: the owner's, or one guest's.
pub fn subscriptions_of(
    node: &Node,
    caller: &crate::guests::Caller,
) -> Result<PushSubscriptionList> {
    vapid_key(node).ok_or_else(not_set_up)?;
    let guest = caller.guest_of();
    Ok(PushSubscriptionList {
        subscriptions: node
            .store()
            .web_subscriptions()
            .into_iter()
            .filter(|one| one.guest == guest)
            .map(view)
            .collect(),
    })
}

/// The longest reason kept: what a destination's record says is short, and the node does not keep more than it was given.
const MAX_REASON_CHARS: usize = 200;

/// A failure of a push service to take a notice at a time. The reason is what the adapter gave (the service's host or
/// name and the status it answered), without control characters and cut at `MAX_REASON_CHARS`.
pub(crate) fn failure_at(at: &Timestamp, reason: &str) -> PushFailure {
    PushFailure {
        at: at.clone(),
        reason: reason
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_REASON_CHARS)
            .collect(),
    }
}

/// What a notice sent to a browser's push service came to, for the subscription's own record.
pub(crate) enum Sent<'a> {
    Taken,
    NotTaken(&'a str),
}

/// Keeps, for each subscription, when its push service last took a notice and when it last did not (and why), so that
/// the owner can be told that a browser's notifications fail. A subscription a push service called gone is removed
/// elsewhere and not recorded here. One write for all the results of a pass.
pub(crate) fn record_sends(node: &Node, results: &[(&str, Sent<'_>)]) -> Result<()> {
    if results.is_empty() {
        return Ok(());
    }
    let at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let mut all = node.store().web_subscriptions();
    let mut changed = false;
    for (id, result) in results {
        let Some(one) = all.iter_mut().find(|one| one.id == *id) else {
            continue;
        };
        match result {
            Sent::Taken => one.last_succeeded_at = Some(at.clone()),
            Sent::NotTaken(reason) => one.last_failure = Some(failure_at(&at, reason)),
        }
        changed = true;
    }
    if changed {
        node.store().save_web_subscriptions(&all)?;
    }
    Ok(())
}

/// The subscriptions that get this notice: of its kind, and made with the node's current key (one made
/// with another key is refused by the push service, and the screen tells the owner to subscribe again).
/// The browsers of `whose` (the owner's for `None`, else that guest's) that want this notice.
pub(crate) fn wanting(
    node: &Node,
    notice: &Notice,
    whose: Option<&agency_proto::client::GuestRef>,
) -> Vec<WebSubscription> {
    let Some(key) = vapid_key(node) else {
        return vec![];
    };
    node.store()
        .web_subscriptions()
        .into_iter()
        .filter(|one| one.key_id == key.key_id && notice.is_wanted_by(&one.events))
        .filter(|one| one.guest.as_ref() == whose)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_name_is_not_an_address_or_a_form_of_one() {
        for name in [
            "fcm.googleapis.com",
            "a.b",
            "web.push.apple.com",
            "x-y.example",
            "1.example",
            "a1.b2c",
        ] {
            assert!(is_host_name(name), "{name}");
        }
        for not_name in [
            "127.0.0.1",
            "0x7f.1",
            "0x7f.0.0.1",
            "2130706433",
            "0177.0.0.1",
            "127.1",
            "localhost",
            "a.1",
            "a.0x7f",
            "example.com.",
            ".example.com",
            "a..b",
            "-a.example",
            "a-.example",
            "a b.example",
            "exa_mple.com",
            "a@b.example",
            "a.b:8443",
            "",
            "::1",
            "[::1]",
        ] {
            assert!(!is_host_name(not_name), "{not_name}");
        }
        assert!(
            !is_host_name(&format!("{}.com", "a".repeat(64))),
            "a label is at most 63 characters"
        );
    }

    #[test]
    fn an_entry_of_the_list_is_a_lowercase_name_or_a_wildcard_domain() {
        for entry in [
            "push.acme.example",
            "*.edge.acme.example",
            "*.push.apple.com",
        ] {
            assert!(is_host_pattern(entry), "{entry}");
        }
        for entry in [
            "*",
            "*.com",
            "*.",
            "**.acme.example",
            "Push.Acme.example",
            "0x7f.1",
            "10.0.0.1",
            "*.127.0.0.1",
            "*.0x7f.1",
            "acme.example/x",
            "",
        ] {
            assert!(!is_host_pattern(entry), "{entry}");
        }
    }

    #[test]
    fn a_failure_keeps_its_time_and_a_short_reason_without_control_characters() {
        let at = Timestamp::parse("2026-11-24T12:00:00Z").unwrap();
        let kept = failure_at(&at, "push.example answered 503");
        assert_eq!(
            (kept.at.clone(), kept.reason.as_str()),
            (at.clone(), "push.example answered 503")
        );
        assert_eq!(
            failure_at(&at, "a\u{0}b\nc\u{1b}[31md").reason,
            "abc[31md",
            "no control character is kept: a line or an escape cannot reach a log or a screen through it"
        );
        let long = "x".repeat(MAX_REASON_CHARS + 50);
        assert_eq!(
            failure_at(&at, &long).reason.chars().count(),
            MAX_REASON_CHARS
        );
        let exact = "y".repeat(MAX_REASON_CHARS);
        assert_eq!(
            failure_at(&at, &exact).reason,
            exact,
            "what fits is kept whole"
        );
    }
}
