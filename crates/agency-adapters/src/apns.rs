//! Apple Push Notification service with token-based authentication: each request carries a provider
//! JWT (ES256, signed with the `.p8` key, reused for 50 minutes) and goes over HTTP/2 to the production
//! or sandbox host that matches the device token.
use agency_core::config::PushConfig;
use agency_core::notice::recovery_text;
use agency_core::ports::{Clock, Notifier};
use agency_core::push::{Notice, NoticeKind, PushError};
use agency_proto::client::{PushEnvironment, PushToken};
use agency_proto::clock::{Duration, OffsetDateTime};
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Apple accepts a provider token for an hour and refuses one renewed more often than every 20 minutes.
const TOKEN_LIFETIME_MINUTES: i64 = 50;

/// Where notifications go. Tests point both at a local server.
#[derive(Clone, Debug)]
pub struct ApnsHosts {
    pub production: String,
    pub sandbox: String,
}

impl Default for ApnsHosts {
    fn default() -> Self {
        Self {
            production: "https://api.push.apple.com".into(),
            sandbox: "https://api.sandbox.push.apple.com".into(),
        }
    }
}

pub struct Apns {
    http: reqwest::Client,
    config: PushConfig,
    key: EncodingKey,
    hosts: ApnsHosts,
    clock: Arc<dyn Clock>,
    /// The provider token and when it was made.
    provider: Mutex<Option<(String, OffsetDateTime)>>,
}

/// The key inside a `.p8` file: PKCS#8 DER between the PEM markers.
fn pkcs8_der(pem: &str) -> Result<Vec<u8>, String> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    STANDARD
        .decode(body.trim())
        .map_err(|_| "the APNs key file is not a PEM .p8 key".to_owned())
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    iat: i64,
}

impl Apns {
    /// Reads the key named in `config`. A key that cannot be read is an error, so a misconfigured node
    /// says so at start instead of silently sending nothing.
    pub fn new(
        config: PushConfig,
        hosts: ApnsHosts,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let pem = std::fs::read_to_string(&config.key_path).map_err(|error| {
            format!(
                "cannot read the APNs key {}: {error}",
                config.key_path.display()
            )
        })?;
        let key = EncodingKey::from_ec_der(&pkcs8_der(&pem)?);
        let http = reqwest::Client::builder()
            .user_agent(crate::USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            http,
            config,
            key,
            hosts,
            clock,
            provider: Mutex::new(None),
        })
    }

    /// The provider token, made again once it is 50 minutes old or after APNs refused it.
    fn provider_token(&self) -> Result<String, PushError> {
        let now = self.clock.now();
        let mut cached = self
            .provider
            .lock()
            .map_err(|_| PushError::Failed("the APNs token is poisoned".into()))?;
        if let Some((token, made)) = cached.as_ref() {
            if now - *made < Duration::minutes(TOKEN_LIFETIME_MINUTES) {
                return Ok(token.clone());
            }
        }
        let header = Header {
            kid: Some(self.config.key_id.clone()),
            ..Header::new(Algorithm::ES256)
        };
        let claims = Claims {
            iss: &self.config.team_id,
            iat: now.unix_timestamp(),
        };
        let token = jsonwebtoken::encode(&header, &claims, &self.key)
            .map_err(|error| PushError::Failed(format!("cannot sign the APNs token: {error}")))?;
        *cached = Some((token.clone(), now));
        Ok(token)
    }

    fn forget_provider_token(&self) {
        if let Ok(mut cached) = self.provider.lock() {
            *cached = None;
        }
    }

    fn host(&self, environment: PushEnvironment) -> &str {
        match environment {
            PushEnvironment::Production => &self.hosts.production,
            PushEnvironment::Sandbox => &self.hosts.sandbox,
        }
    }
}

/// An alert the app words itself: the key of a string it ships, and the one text it fills in (none when
/// the node leaves the text out, which picks the string that names no title).
fn localized(key: &str, no_text_key: &str, text: Option<&String>) -> Value {
    match text {
        Some(text) => json!({ "loc-key": key, "loc-args": [text] }),
        None => json!({ "loc-key": no_text_key }),
    }
}

/// The notification. A request keeps its alert as written (the team's label as the title and the request's
/// title as the body) and the `agency` keys the app opens it with, and has no `type`; a device or a
/// refused decision is an alert the app words from its own strings, with `agency.type` telling the app
/// which screen to open. Every kind carries the badge and is grouped by its thread.
pub fn payload(notice: &Notice) -> Value {
    let (alert, agency) = match &notice.kind {
        NoticeKind::Request {
            org,
            team,
            request,
            team_label: Some(label),
            title: Some(title),
        } => (
            json!({ "title": label, "body": title }),
            json!({ "org": org, "team": team, "request": request }),
        ),
        NoticeKind::Request {
            org, team, request, ..
        } => (
            json!({ "loc-key": "PUSH_REQUEST_WAITING_HIDDEN" }),
            json!({ "org": org, "team": team, "request": request }),
        ),
        NoticeKind::Device { label, .. } => (
            localized(
                "PUSH_DEVICE_WAITING",
                "PUSH_DEVICE_WAITING_NO_LABEL",
                label.as_ref(),
            ),
            json!({ "type": "device" }),
        ),
        NoticeKind::Decision {
            org,
            team,
            id,
            title,
        } => (
            localized(
                "PUSH_DECISION_REFUSED",
                "PUSH_DECISION_REFUSED_NO_TITLE",
                title.as_ref(),
            ),
            json!({ "type": "decision", "org": org, "team": team, "id": id }),
        ),
        NoticeKind::Recovery {
            org,
            takes_effect_at,
        } => {
            let (title, body) = recovery_text(org, takes_effect_at.as_deref());
            (
                json!({ "title": title, "body": body }),
                json!({ "type": "recovery", "org": org }),
            )
        }
    };
    let mut aps = json!({
        "alert": alert,
        "sound": "default",
        "thread-id": notice.thread(),
        "badge": notice.badge,
    });
    if matches!(notice.kind, NoticeKind::Request { .. }) {
        // A request starting to wait changes the desk the widgets show: this wakes the app to read it, when
        // the system allows, so that no push of its own is needed for the same change.
        aps["content-available"] = json!(1);
    }
    json!({ "aps": aps, "agency": agency })
}

/// The push that only wakes the app to read the desk: no alert, no sound, no badge, and nothing of the
/// desk, not a title, a label or a count.
pub fn desk_payload() -> Value {
    json!({ "aps": { "content-available": 1 }, "agency": { "type": "desk" } })
}

/// A background push that no one has seen is worth nothing an hour later.
const DESK_EXPIRES_MINUTES: i64 = 60;
/// What replaces an earlier desk push the device has not received yet.
const DESK_COLLAPSE_ID: &str = "desk";

/// How one push is sent: its type, priority, what it replaces, and when it stops being worth sending.
struct Delivery<'a> {
    push_type: &'a str,
    priority: &'a str,
    collapse: String,
    expiration: Option<i64>,
}

impl Apns {
    async fn deliver(
        &self,
        token: &PushToken,
        delivery: Delivery<'_>,
        body: &Value,
    ) -> Result<(), PushError> {
        let provider = self.provider_token()?;
        let url = format!("{}/3/device/{}", self.host(token.environment), token.token);
        let mut request = self
            .http
            .post(url)
            .header("authorization", format!("bearer {provider}"))
            .header("apns-topic", &self.config.bundle_id)
            .header("apns-push-type", delivery.push_type)
            .header("apns-priority", delivery.priority)
            .header("apns-collapse-id", delivery.collapse);
        if let Some(expiration) = delivery.expiration {
            request = request.header("apns-expiration", expiration.to_string());
        }
        let response = request
            .json(body)
            .send()
            .await
            .map_err(|error| PushError::Failed(format!("APNs is unreachable: {error}")))?;
        let status = response.status().as_u16();
        if status == 200 {
            return Ok(());
        }
        let reason = response
            .json::<Value>()
            .await
            .ok()
            .and_then(|body| body["reason"].as_str().map(str::to_owned))
            .unwrap_or_default();
        match (status, reason.as_str()) {
            (410, _) | (400, "BadDeviceToken") => Err(PushError::Gone),
            (403, "ExpiredProviderToken" | "InvalidProviderToken") => {
                self.forget_provider_token();
                Err(PushError::Failed(format!(
                    "APNs refused the provider token ({reason})"
                )))
            }
            _ => Err(PushError::Failed(format!(
                "APNs answered {status} {reason}"
            ))),
        }
    }
}

#[async_trait]
impl Notifier for Apns {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError> {
        let delivery = Delivery {
            push_type: "alert",
            priority: "10",
            collapse: notice.collapse_id().chars().take(64).collect(),
            expiration: None,
        };
        self.deliver(token, delivery, &payload(notice)).await
    }

    /// Apple requires priority 5 for a push that only has `content-available`: 10 is refused.
    async fn send_desk_changed(&self, token: &PushToken) -> Result<(), PushError> {
        let expires = self.clock.now() + Duration::minutes(DESK_EXPIRES_MINUTES);
        let delivery = Delivery {
            push_type: "background",
            priority: "5",
            collapse: DESK_COLLAPSE_ID.to_owned(),
            expiration: Some(expires.unix_timestamp()),
        };
        self.deliver(token, delivery, &desk_payload()).await
    }
}
