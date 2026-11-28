//! The sign-in provider in front of the Client API: Cloudflare Access, or an OpenID Connect provider (`oidc`).
//! Either sends every signed-in request with a JWT signed by the provider's keys: Access in
//! `Cf-Access-Jwt-Assertion`, an OIDC provider in the header its setting names. The node checks the signature
//! (RS256 only), the audience, the issuer, and the validity period on every request, the same way for both, so a
//! missing or wrong setup fails closed.
use agency_core::config::AccessConfig;
use agency_core::oidc_config::{is_https_url, OidcConfig, DEFAULT_TOKEN_HEADER};
use agency_core::ports::Clock;
use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
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

/// Where the provider's documents come from: the key set (Access's `https://<team domain>/cdn-cgi/access/certs`,
/// an OIDC provider's `jwks_uri`) and an OIDC provider's discovery document, each a JSON document at an `https`
/// address.
#[async_trait]
pub trait CertSource: Send + Sync {
    async fn certs(&self, url: &str) -> Result<Value, String>;
}

pub struct HttpCerts(reqwest::Client);

impl HttpCerts {
    pub fn new() -> reqwest::Result<Self> {
        Ok(Self(
            reqwest::Client::builder()
                .user_agent(crate::USER_AGENT)
                // Others choose where this node sends (team places, invite links, team domains); a
                // redirect would send its requests, and their headers, somewhere else.
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()?,
        ))
    }
}

#[async_trait]
impl CertSource for HttpCerts {
    async fn certs(&self, url: &str) -> Result<Value, String> {
        let response = self
            .0
            .get(url)
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

/// What the node checks a sign-in against: Access's setting or an OIDC provider's, in one form.
#[derive(Clone, Debug)]
pub struct Provider {
    public_host: String,
    issuer: String,
    audience: String,
    owner_email: String,
    /// The key set's address, or none when it is read from the issuer's discovery document.
    keys_url: Option<String>,
    token_header: String,
    /// Whether the header is `Authorization`, read as `Bearer <token>`.
    is_bearer: bool,
    email_claim: String,
    /// The Access setting, for what only Access has (the app sign-in handoff).
    access: Option<AccessConfig>,
    /// `Access` or `OIDC`, as the node's messages name the provider.
    name: &'static str,
}

impl Provider {
    pub fn access(config: AccessConfig) -> Self {
        Self {
            public_host: config.public_host.clone(),
            issuer: format!("https://{}", config.team_domain),
            audience: config.audience.clone(),
            owner_email: config.owner_email.clone(),
            keys_url: Some(format!(
                "https://{}/cdn-cgi/access/certs",
                config.team_domain
            )),
            token_header: crate::handoff::ACCESS_HEADER.to_owned(),
            is_bearer: false,
            email_claim: "email".to_owned(),
            access: Some(config),
            name: "Access",
        }
    }

    pub fn oidc(config: &OidcConfig) -> Self {
        let token_header = config.token_header();
        Self {
            public_host: config.public_host.clone(),
            issuer: config.issuer.clone(),
            audience: config.audience.clone(),
            owner_email: config.owner_email.clone(),
            keys_url: config.keys_url.clone(),
            is_bearer: token_header == DEFAULT_TOKEN_HEADER,
            token_header,
            email_claim: config.email_claim().to_owned(),
            access: None,
            name: "OIDC",
        }
    }

    /// `Access` or `OIDC`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The host name requests from outside name.
    pub fn public_host(&self) -> &str {
        &self.public_host
    }

    /// Only this signed-in email may write.
    pub fn owner_email(&self) -> &str {
        &self.owner_email
    }

    /// The Access setting, when the provider is Access.
    pub fn access_config(&self) -> Option<&AccessConfig> {
        self.access.as_ref()
    }

    /// The token a request carries in the provider's header: `Bearer <token>` in `Authorization`, the whole
    /// value in any other header.
    pub fn token<'a>(&self, headers: &'a axum::http::HeaderMap) -> Option<&'a str> {
        let value = headers.get(self.token_header.as_str())?.to_str().ok()?;
        if !self.is_bearer {
            return Some(value);
        }
        let (scheme, token) = value.split_once(' ')?;
        scheme
            .eq_ignore_ascii_case("bearer")
            .then(|| token.trim())
            .filter(|token| !token.is_empty())
    }
}

/// Who the provider let in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub email: Option<String>,
}

enum Check {
    Passed(Identity),
    UnknownKey,
    Failed(String),
}

pub struct SignInVerifier {
    provider: Provider,
    source: Arc<dyn CertSource>,
    clock: Arc<dyn Clock>,
    keys: Mutex<(HashMap<String, DecodingKey>, Option<i64>)>,
}

impl SignInVerifier {
    pub fn new(provider: Provider, source: Arc<dyn CertSource>, clock: Arc<dyn Clock>) -> Self {
        Self {
            provider,
            source,
            clock,
            keys: Mutex::new((HashMap::new(), None)),
        }
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    fn check_claims(&self, claims: &serde_json::Map<String, Value>) -> Result<Identity, String> {
        let now = self.clock.now().unix_timestamp();
        let audiences = match claims.get("aud") {
            Some(Value::Array(all)) => all.iter().filter_map(Value::as_str).collect(),
            Some(Value::String(one)) => vec![one.as_str()],
            _ => vec![],
        };
        if !audiences.contains(&self.provider.audience.as_str()) {
            return Err("wrong audience".into());
        }
        if claims.get("iss").and_then(Value::as_str) != Some(self.provider.issuer.as_str()) {
            return Err("wrong issuer".into());
        }
        // A time claim that is there must be an integer: a string, a float, null, or an object never passes
        // as "no limit".
        let time = |name: &str| match claims.get(name) {
            None => Ok(None),
            Some(value) => value
                .as_i64()
                .map(Some)
                .ok_or_else(|| format!("{name} is not an integer")),
        };
        if time("exp")?.is_none_or(|exp| exp + SKEW_SECONDS < now) {
            return Err("expired".into());
        }
        if time("nbf")?.is_some_and(|nbf| nbf - SKEW_SECONDS > now) {
            return Err("not valid yet".into());
        }
        let email = claims
            .get(&self.provider.email_claim)
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(Identity { email })
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
        match jsonwebtoken::decode::<serde_json::Map<String, Value>>(token, key, &validation) {
            Ok(data) => self
                .check_claims(&data.claims)
                .map_or_else(Check::Failed, Check::Passed),
            Err(_) => Check::Failed("the signature does not match".into()),
        }
    }

    /// The key set's address: the one the setting names, else the `jwks_uri` of the issuer's discovery
    /// document, which must be `https` as the issuer is.
    async fn keys_url(&self) -> Result<String, String> {
        if let Some(url) = &self.provider.keys_url {
            return Ok(url.clone());
        }
        let discovery = format!(
            "{}/.well-known/openid-configuration",
            self.provider.issuer.trim_end_matches('/')
        );
        let document = self.source.certs(&discovery).await?;
        let url = document["jwks_uri"]
            .as_str()
            .ok_or("the discovery document names no jwks_uri")?;
        if !is_https_url(url) {
            return Err(
                "the discovery document's jwks_uri is not https://<host>[:<port>][/<path>]".into(),
            );
        }
        Ok(url.to_owned())
    }

    /// Fetches the keys again, unless they were fetched in the last minute.
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
        let jwks = self.source.certs(&self.keys_url().await?).await?;
        self.keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0 = keys_from_jwks(&jwks);
        Ok(())
    }

    /// Checks a sign-in token. An unknown key id fetches the keys again (keys rotate).
    pub async fn verify(&self, token: Option<&str>) -> Result<Identity, String> {
        let name = self.provider.name;
        let token = token.ok_or_else(|| format!("no {name} token"))?;
        match self.check(token) {
            Check::Passed(identity) => return Ok(identity),
            Check::Failed(reason) => return Err(reason),
            Check::UnknownKey => {}
        }
        self.refetch()
            .await
            .map_err(|error| format!("cannot fetch the {name} certs: {error}"))?;
        match self.check(token) {
            Check::Passed(identity) => Ok(identity),
            Check::Failed(reason) => Err(reason),
            Check::UnknownKey => Err("unknown signing key".into()),
        }
    }
}
