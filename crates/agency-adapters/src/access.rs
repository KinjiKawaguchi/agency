//! Cloudflare Access in front of the Client API. Access sends every signed-in request with a JWT in
//! `Cf-Access-Jwt-Assertion`, signed with the team's keys. The node checks the signature (RS256 only),
//! the audience, the issuer, and the validity period on every request, so a missing or wrong Access
//! setup fails closed.
use agency_core::config::AccessConfig;
use agency_core::ports::Clock;
use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Clock skew allowed on `exp` and `nbf`.
const SKEW_SECONDS: i64 = 60;
/// Certs are fetched again for an unknown key id at most this often.
const REFETCH_SECONDS: i64 = 60;

/// Whether the Access settings have the expected shape. A malformed setting is treated as no setting.
pub fn is_valid_config(config: &AccessConfig) -> bool {
    let is_label = |label: &str| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let is_host = |host: &str| host.contains('.') && host.split('.').all(is_label);
    let team = config.team_domain.strip_suffix(".cloudflareaccess.com");
    let is_audience = config.audience.len() == 64
        && config
            .audience
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let is_email = config
        .owner_email
        .split_once('@')
        .is_some_and(|(user, domain)| !user.is_empty() && is_host(domain));
    is_host(&config.public_host) && team.is_some_and(is_label) && is_audience && is_email
}

/// Where the team's signing keys come from (`https://<team domain>/cdn-cgi/access/certs`).
#[async_trait]
pub trait CertSource: Send + Sync {
    async fn certs(&self, team_domain: &str) -> Result<Value, String>;
}

pub struct HttpCerts(reqwest::Client);

impl HttpCerts {
    pub fn new() -> reqwest::Result<Self> {
        Ok(Self(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
        ))
    }
}

#[async_trait]
impl CertSource for HttpCerts {
    async fn certs(&self, team_domain: &str) -> Result<Value, String> {
        let response = self
            .0
            .get(format!("https://{team_domain}/cdn-cgi/access/certs"))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let response = response
            .error_for_status()
            .map_err(|error| error.to_string())?;
        response.json().await.map_err(|error| error.to_string())
    }
}

/// RSA keys from a JWKS document, by key id.
pub fn keys_from_jwks(jwks: &Value) -> HashMap<String, DecodingKey> {
    let keys = jwks["keys"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let rsa = keys
        .iter()
        .filter(|key| key["kty"] == "RSA")
        .filter_map(|key| {
            let kid = key["kid"].as_str()?.to_owned();
            let decoding =
                DecodingKey::from_rsa_components(key["n"].as_str()?, key["e"].as_str()?).ok()?;
            Some((kid, decoding))
        });
    rsa.collect()
}

#[derive(Deserialize)]
struct Claims {
    #[serde(default)]
    aud: Value,
    #[serde(default)]
    iss: String,
    exp: Option<i64>,
    nbf: Option<i64>,
    email: Option<String>,
}

/// Who Access let in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub email: Option<String>,
}

enum Check {
    Passed(Identity),
    UnknownKey,
    Failed(String),
}

pub struct AccessVerifier {
    config: AccessConfig,
    source: Arc<dyn CertSource>,
    clock: Arc<dyn Clock>,
    keys: Mutex<(HashMap<String, DecodingKey>, Option<i64>)>,
}

impl AccessVerifier {
    pub fn new(config: AccessConfig, source: Arc<dyn CertSource>, clock: Arc<dyn Clock>) -> Self {
        Self {
            config,
            source,
            clock,
            keys: Mutex::new((HashMap::new(), None)),
        }
    }

    pub fn config(&self) -> &AccessConfig {
        &self.config
    }

    fn check_claims(&self, claims: Claims) -> Result<Identity, String> {
        let now = self.clock.now().unix_timestamp();
        let audiences = match &claims.aud {
            Value::Array(all) => all.iter().filter_map(Value::as_str).collect(),
            Value::String(one) => vec![one.as_str()],
            _ => vec![],
        };
        if !audiences.contains(&self.config.audience.as_str()) {
            return Err("wrong audience".into());
        }
        if claims.iss != format!("https://{}", self.config.team_domain) {
            return Err("wrong issuer".into());
        }
        if claims.exp.is_none_or(|exp| exp + SKEW_SECONDS < now) {
            return Err("expired".into());
        }
        if claims.nbf.is_some_and(|nbf| nbf - SKEW_SECONDS > now) {
            return Err("not valid yet".into());
        }
        Ok(Identity {
            email: claims.email,
        })
    }

    fn check(&self, token: &str) -> Check {
        let Ok(header) = jsonwebtoken::decode_header(token) else {
            return Check::Failed("unreadable token".into());
        };
        let keys = self
            .keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(key) = header.kid.as_ref().and_then(|kid| keys.0.get(kid)) else {
            return Check::UnknownKey;
        };
        // Only RS256: a token in any other algorithm (such as HS256 keyed with the public key) fails.
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = false;
        validation.validate_aud = false;
        validation.required_spec_claims.clear();
        match jsonwebtoken::decode::<Claims>(token, key, &validation) {
            Ok(data) => self
                .check_claims(data.claims)
                .map_or_else(Check::Failed, Check::Passed),
            Err(_) => Check::Failed("the signature does not match".into()),
        }
    }

    /// Fetches the certs again, unless they were fetched in the last minute.
    async fn refetch(&self) -> Result<(), String> {
        let now = self.clock.now().unix_timestamp();
        {
            let mut keys = self
                .keys
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if keys.1.is_some_and(|at| now - at < REFETCH_SECONDS) {
                return Ok(());
            }
            keys.1 = Some(now);
        }
        let jwks = self.source.certs(&self.config.team_domain).await?;
        self.keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0 = keys_from_jwks(&jwks);
        Ok(())
    }

    /// Checks an Access JWT. An unknown key id fetches the certs again (keys rotate).
    pub async fn verify(&self, token: Option<&str>) -> Result<Identity, String> {
        let token = token.ok_or("no Access token")?;
        match self.check(token) {
            Check::Passed(identity) => return Ok(identity),
            Check::Failed(reason) => return Err(reason),
            Check::UnknownKey => {}
        }
        self.refetch()
            .await
            .map_err(|error| format!("cannot fetch the Access certs: {error}"))?;
        match self.check(token) {
            Check::Passed(identity) => Ok(identity),
            Check::Failed(reason) => Err(reason),
            Check::UnknownKey => Err("unknown signing key".into()),
        }
    }
}
