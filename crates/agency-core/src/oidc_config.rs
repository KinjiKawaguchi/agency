//! `oidc` in `config.json`: an OpenID Connect provider in front of the Client API, in place of Cloudflare Access.
//! The node checks its signed sign-in tokens with the same rules as Access tokens (docs/protocol.md, "Who is let
//! in"); how requests reach the node from outside is the deployment's choice.
use serde::{Deserialize, Serialize};

/// The header the token comes in when the setting names none, read as `Bearer <token>`.
pub const DEFAULT_TOKEN_HEADER: &str = "authorization";
/// The claim that holds the signed-in email when the setting names none.
pub const DEFAULT_EMAIL_CLAIM: &str = "email";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct OidcConfig {
    /// The host name requests from outside name, such as `agency.example.com`.
    pub public_host: String,
    /// The provider's issuer, `https://<host>[:<port>][/<path>]`, exactly as its tokens' `iss` says it.
    pub issuer: String,
    /// The audience the tokens must name (`aud`): the client id the node is registered under.
    pub audience: String,
    /// The owner's subject at the issuer (`sub`): only a token of this issuer and this subject is the owner's,
    /// and may write as the node's account. An email names no owner here, since a provider may let people
    /// type their own.
    pub owner_subject: String,
    /// Where the provider's signing keys are (a JWKS document). Left out, it is read from the issuer's
    /// `/.well-known/openid-configuration` (`jwks_uri`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys_url: Option<String>,
    /// The request header the token comes in (default `authorization`, read as `Bearer <token>`). Another
    /// header holds the token alone, as a proxy in front of the node sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_header: Option<String>,
    /// The claim that holds the signed-in email (default `email`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_claim: Option<String>,
}

impl OidcConfig {
    pub fn new(
        public_host: impl Into<String>,
        issuer: impl Into<String>,
        audience: impl Into<String>,
        owner_subject: impl Into<String>,
    ) -> Self {
        Self {
            public_host: public_host.into(),
            issuer: issuer.into(),
            audience: audience.into(),
            owner_subject: owner_subject.into(),
            keys_url: None,
            token_header: None,
            email_claim: None,
        }
    }

    /// The same setting with the key set at `url` instead of the one discovery names.
    pub fn with_keys_url(self, url: impl Into<String>) -> Self {
        Self {
            keys_url: Some(url.into()),
            ..self
        }
    }

    /// The same setting, reading the token from `header`.
    pub fn with_token_header(self, header: impl Into<String>) -> Self {
        Self {
            token_header: Some(header.into()),
            ..self
        }
    }

    /// The same setting, reading the email from `claim`.
    pub fn with_email_claim(self, claim: impl Into<String>) -> Self {
        Self {
            email_claim: Some(claim.into()),
            ..self
        }
    }

    /// The header the token comes in, lowercased.
    pub fn token_header(&self) -> String {
        self.token_header
            .as_deref()
            .unwrap_or(DEFAULT_TOKEN_HEADER)
            .to_ascii_lowercase()
    }

    pub fn email_claim(&self) -> &str {
        self.email_claim.as_deref().unwrap_or(DEFAULT_EMAIL_CLAIM)
    }

    /// Why the setting cannot be used, naming the field, in words for the owner.
    pub fn check(&self) -> Result<(), String> {
        let field = |name: &str, what: &str| Err(format!("oidc.{name} {what}"));
        if !is_host(&self.public_host) {
            return field("publicHost", "is not a host name");
        }
        if !is_https_url(&self.issuer) {
            return field("issuer", "is not https://<host>[:<port>][/<path>]");
        }
        if !is_token_text(&self.audience) {
            return field(
                "audience",
                "is empty or holds a space or a control character",
            );
        }
        if !is_token_text(&self.owner_subject) {
            return field(
                "ownerSubject",
                "is empty or holds a space or a control character",
            );
        }
        if self
            .keys_url
            .as_deref()
            .is_some_and(|url| !is_https_url(url))
        {
            return field("keysUrl", "is not https://<host>[:<port>][/<path>]");
        }
        if self
            .token_header
            .as_deref()
            .is_some_and(|name| !is_header_name(name))
        {
            return field("tokenHeader", "is not a header name");
        }
        if self
            .email_claim
            .as_deref()
            .is_some_and(|claim| !is_token_text(claim))
        {
            return field(
                "emailClaim",
                "is empty or holds a space or a control character",
            );
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        self.check().is_ok()
    }
}

fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

/// A lowercase host name of labels (`localhost` included), or an IPv4 address.
fn is_host(host: &str) -> bool {
    host.len() <= 253 && host.split('.').all(is_label)
}

/// `https://<host>[:<port>][/<path>]`, with no user information, query, or fragment: the only form of the
/// issuer and of the key set's address the node reads keys from.
pub fn is_https_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let is_port = port.is_none_or(|port| {
        !port.is_empty()
            && port.bytes().all(|b| b.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|port| port > 0)
    });
    let is_path = path
        .bytes()
        .all(|b| b.is_ascii_graphic() && !matches!(b, b'?' | b'#' | b'\\'));
    is_host(host) && is_port && is_path
}

fn is_token_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= 256 && text.bytes().all(|b| b.is_ascii_graphic())
}

fn is_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> OidcConfig {
        OidcConfig::new(
            "agency.example.com",
            "https://login.example.com/tenant/v2.0",
            "app-client-id",
            "00000000-0000-0000-0000-00000000a11c",
        )
    }

    #[test]
    fn a_complete_setting_is_valid_and_its_defaults_apply() {
        assert_eq!(valid().check(), Ok(()));
        assert_eq!(valid().token_header(), "authorization");
        assert_eq!(valid().email_claim(), "email");
        let named = valid()
            .with_keys_url("https://localhost:8443/keys")
            .with_token_header("X-Auth-Request-Access-Token")
            .with_email_claim("preferred_username");
        assert_eq!(named.check(), Ok(()));
        assert_eq!(named.token_header(), "x-auth-request-access-token");
        assert_eq!(named.email_claim(), "preferred_username");
    }

    #[test]
    fn an_issuer_or_key_set_address_must_be_https_with_nothing_but_a_host_port_and_path() {
        for good in [
            "https://login.example.com",
            "https://login.example.com/tenant/v2.0",
            "https://localhost:8443/realms/acme",
            "https://127.0.0.1:9000",
        ] {
            assert!(is_https_url(good), "{good}");
        }
        for bad in [
            "http://login.example.com",
            "https://user@login.example.com",
            "https://login.example.com/?x=1",
            "https://login.example.com/#frag",
            "https://login.example.com:0",
            "https://login.example.com:99999",
            "https://login.example.com:",
            "https://Login.Example.com",
            "https://login.example.com/a b",
            "https://login.example.com\\evil",
            "https://",
        ] {
            assert!(!is_https_url(bad), "{bad}");
        }
    }

    #[test]
    fn a_malformed_field_is_named() {
        let named = |config: OidcConfig| config.check().unwrap_err();
        let mut config = valid();
        config.public_host = "agency example".into();
        assert!(named(config).starts_with("oidc.publicHost"));
        let mut config = valid();
        config.issuer = "http://login.example.com".into();
        assert!(named(config).starts_with("oidc.issuer"));
        let mut config = valid();
        config.audience = String::new();
        assert!(named(config).starts_with("oidc.audience"));
        let mut config = valid();
        config.owner_subject = "a b".into();
        assert!(named(config).starts_with("oidc.ownerSubject"));
        assert!(named(valid().with_keys_url("http://keys")).starts_with("oidc.keysUrl"));
        assert!(named(valid().with_token_header("x auth")).starts_with("oidc.tokenHeader"));
        assert!(named(valid().with_email_claim("")).starts_with("oidc.emailClaim"));
    }

    #[test]
    fn an_unknown_field_is_refused() {
        let text = r#"{"publicHost":"a.example.com","issuer":"https://l.example.com","audience":"x","ownerSubject":"s-1","secret":"s"}"#;
        assert!(serde_json::from_str::<OidcConfig>(text).is_err());
    }
}
