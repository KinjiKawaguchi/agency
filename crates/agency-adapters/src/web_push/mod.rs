//! Web Push to the owner's browsers (RFC 8030, 8291, 8292). Each message is encrypted to one browser and
//! signed with the node's VAPID key; the browser's push service, which the browser chose, carries it. The
//! node sends only where `check_endpoint` allows, never follows a redirect, never connects to an address
//! that is not on the public internet, and sends no credentials besides the VAPID header.
mod ece;
mod network;
mod vapid;

use agency_core::config::WebPushConfig;
use agency_core::ports::{Clock, Notifier};
use agency_core::push::{Notice, NoticeKind, PushError};
use agency_core::web_push::{check_endpoint, WebSubscription};
use agency_proto::client::{PushToken, VapidKey};
use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use vapid::Vapid;

/// How long a push service keeps a message for a browser that is off.
const TTL_SECONDS: &str = "3600";
/// A title or a label longer than this is cut, so a message always fits the one record.
const TEXT_CHARS: usize = 200;

pub struct WebPush {
    http: reqwest::Client,
    vapid: Vapid,
    subject: String,
    extra_hosts: Vec<String>,
    clock: Arc<dyn Clock>,
    /// Only a test of this module sets it: the endpoint is then not checked and the address not filtered,
    /// so that a local server can stand for a push service.
    is_open_for_tests: bool,
}

/// The HTTP client of the node: `https` only, no redirect, names resolved to public addresses only. A test
/// that needs a local server to stand for a push service asks for one with none of the last and the first.
fn client(is_open_for_tests: bool) -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder()
        .user_agent(crate::USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10));
    let builder = if is_open_for_tests {
        builder
    } else {
        builder
            .https_only(true)
            .dns_resolver(Arc::new(network::PublicOnly))
    };
    builder.build().map_err(|error| error.to_string())
}

/// Whether a subscription's keys are a browser's: `p256dh` a point on P-256 (65 octets), `auth` 16 octets,
/// both base64url. Checked when the subscription is registered, so a key that no message can ever be
/// encrypted to is refused then and not at the first send.
pub fn check_keys(p256dh: &str, auth: &str) -> Result<(), String> {
    let point = B64
        .decode(p256dh)
        .ok()
        .and_then(|bytes| p256::PublicKey::from_sec1_bytes(&bytes).ok());
    let is_secret = B64.decode(auth).is_ok_and(|bytes| bytes.len() == 16);
    if point.is_none() || !is_secret {
        return Err("p256dh is not a point of P-256 or auth is not 16 octets".to_owned());
    }
    Ok(())
}

impl WebPush {
    /// Reads the node's VAPID key from `key_path`, or makes it. `subject` is the contact a push service may
    /// reach the node's operator at.
    pub fn new(
        config: &WebPushConfig,
        default_subject: String,
        key_path: &Path,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        Ok(Self {
            http: client(false)?,
            vapid: Vapid::load_or_create(key_path)?,
            subject: config.subject.clone().unwrap_or(default_subject),
            extra_hosts: config.extra_hosts.clone(),
            clock,
            is_open_for_tests: false,
        })
    }

    #[cfg(test)]
    fn for_tests(clock: Arc<dyn Clock>) -> Self {
        Self {
            http: client(true).unwrap(),
            vapid: Vapid::new(p256::SecretKey::from_slice(&[5u8; 32]).unwrap()),
            subject: "mailto:ops@acme.example".to_owned(),
            extra_hosts: vec![],
            clock,
            is_open_for_tests: true,
        }
    }
}

fn cut(text: &str) -> String {
    text.chars().take(TEXT_CHARS).collect()
}

/// What the browser's worker is told, as the iPhone's payload is: the `agency` keys it opens the notice
/// with and the badge, and the text only when the node has it (`hideTitles` leaves it out). A request has
/// a `title` (the team's label) and a `body` (its title); a device or a refused decision has a `detail`
/// (the device's label, the decision's title), which the worker puts after wording of its own.
pub fn payload(notice: &Notice) -> Value {
    let (agency, text) = match &notice.kind {
        NoticeKind::Request {
            org,
            team,
            request,
            team_label,
            title,
        } => (
            json!({ "org": org, "team": team, "request": request }),
            match (team_label, title) {
                (Some(label), Some(title)) => json!({ "title": cut(label), "body": cut(title) }),
                _ => json!({}),
            },
        ),
        NoticeKind::Device { label, .. } => (
            json!({ "type": "device" }),
            json!({ "detail": label.as_deref().map(cut) }),
        ),
        NoticeKind::Decision {
            org,
            team,
            id,
            title,
        } => (
            json!({ "type": "decision", "org": org, "team": team, "id": id }),
            json!({ "detail": title.as_deref().map(cut) }),
        ),
    };
    let mut message = json!({ "agency": agency, "badge": notice.badge });
    if let (Some(all), Some(extra)) = (message.as_object_mut(), text.as_object()) {
        all.extend(
            extra
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
    message
}

/// A topic of at most 32 base64url characters that names the notice, so that a push service replaces an
/// earlier message about the same thing. It is a hash: the request's name is not shown to the service.
fn topic(notice: &Notice) -> String {
    B64.encode(&Sha256::digest(notice.collapse_id().as_bytes())[..18])
}

fn urgency(notice: &Notice) -> &'static str {
    match notice.kind {
        NoticeKind::Request { .. } => "high",
        _ => "normal",
    }
}

/// The scheme and host of an endpoint (its origin, which is the audience of the VAPID header).
fn origin(endpoint: &str) -> String {
    let (scheme, rest) = endpoint.split_once("://").unwrap_or(("https", endpoint));
    format!("{scheme}://{}", rest.split('/').next().unwrap_or(rest))
}

impl WebPush {
    fn endpoint(&self, subscription: &WebSubscription) -> Result<String, PushError> {
        if self.is_open_for_tests {
            return Ok(subscription.endpoint.clone());
        }
        check_endpoint(&subscription.endpoint, &self.extra_hosts).map_err(|_| PushError::Gone)
    }

    /// The encrypted body of the notice for one subscription. Keys that cannot be a browser's make the
    /// subscription gone: no message will ever be taken by it.
    fn body(&self, subscription: &WebSubscription, notice: &Notice) -> Result<Vec<u8>, PushError> {
        let keys = (
            B64.decode(&subscription.p256dh),
            B64.decode(&subscription.auth),
        );
        let (Ok(ua_public), Ok(auth)) = keys else {
            return Err(PushError::Gone);
        };
        let mut salt = [0u8; 16];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut salt);
        let secret = p256::SecretKey::random(&mut rand_core::OsRng);
        let message = payload(notice).to_string();
        ece::encrypt(message.as_bytes(), &ua_public, &auth, &secret, salt).map_err(|error| {
            match error {
                ece::EceError::BadSubscriptionKeys => PushError::Gone,
                ece::EceError::TooLong => {
                    PushError::Failed("a push message is too long".to_owned())
                }
            }
        })
    }
}

impl WebPush {
    pub fn key(&self) -> VapidKey {
        VapidKey {
            public_key: self.vapid.public_key().to_owned(),
            key_id: self.vapid.key_id().to_owned(),
        }
    }

    /// Sends one notice to one subscription. 404 and 410 say the browser unsubscribed; a subscription the
    /// node would no longer send to, or whose keys are not a browser's, is gone as well.
    pub async fn send(
        &self,
        subscription: &WebSubscription,
        notice: &Notice,
    ) -> Result<(), PushError> {
        let endpoint = self.endpoint(subscription)?;
        let body = self.body(subscription, notice)?;
        let host = origin(&endpoint);
        let authorization =
            self.vapid
                .authorization(&host, &self.subject, self.clock.now().unix_timestamp());
        let response = self
            .http
            .post(&endpoint)
            .header("authorization", authorization)
            .header("content-encoding", "aes128gcm")
            .header("content-type", "application/octet-stream")
            .header("ttl", TTL_SECONDS)
            .header("urgency", urgency(notice))
            .header("topic", topic(notice))
            .body(body)
            .send()
            .await
            .map_err(|_| PushError::Failed(format!("{host} is unreachable")))?;
        match response.status().as_u16() {
            200..=299 => Ok(()),
            404 | 410 => Err(PushError::Gone),
            status => Err(PushError::Failed(format!("{host} answered {status}"))),
        }
    }
}

/// The node's notifier when Web Push is set up: APNs (or nothing) for tokens, and a browser's push
/// service for subscriptions.
pub struct WithWebPush {
    pub tokens: Arc<dyn Notifier>,
    pub web: WebPush,
}

#[async_trait]
impl Notifier for WithWebPush {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError> {
        self.tokens.send(token, notice).await
    }

    fn web_push_key(&self) -> Option<VapidKey> {
        Some(self.web.key())
    }

    fn web_push_check_keys(&self, p256dh: &str, auth: &str) -> Result<(), String> {
        check_keys(p256dh, auth)
    }

    async fn send_web(
        &self,
        subscription: &WebSubscription,
        notice: &Notice,
    ) -> Result<(), PushError> {
        self.web.send(subscription, notice).await
    }
}

#[cfg(test)]
mod tests;
