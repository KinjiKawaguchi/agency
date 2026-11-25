//! Apple Push Notification service with token-based authentication: each request carries a provider
//! JWT (ES256, signed with the `.p8` key, reused for 50 minutes) and goes over HTTP/2 to the production
//! or sandbox host that matches the device token.
use agency_core::config::PushConfig;
use agency_core::ports::{Clock, Notifier};
use agency_core::push::{Notice, PushError};
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

/// The notification: the team label as the title, the request title as the body, grouped by team, and
/// the `agency` keys the app opens the request with.
pub fn payload(notice: &Notice) -> Value {
    json!({
        "aps": {
            "alert": { "title": notice.title, "body": notice.body },
            "sound": "default",
            "thread-id": notice.thread(),
        },
        "agency": { "org": notice.org, "team": notice.team, "request": notice.request },
    })
}

#[async_trait]
impl Notifier for Apns {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError> {
        let provider = self.provider_token()?;
        let url = format!("{}/3/device/{}", self.host(token.environment), token.token);
        let collapse: String = notice.request.chars().take(64).collect();
        let response = self
            .http
            .post(url)
            .header("authorization", format!("bearer {provider}"))
            .header("apns-topic", &self.config.bundle_id)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header("apns-collapse-id", collapse)
            .json(&payload(notice))
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
