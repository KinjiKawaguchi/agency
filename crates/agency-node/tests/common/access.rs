//! Cloudflare Access for the integration tests: a test-only signing key, the certs that publish it, and
//! a node behind Access.
use super::{id, memory_runtime, node_with, Running};
use agency_adapters::access::CertSource;
use agency_core::config::AccessConfig;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

/// A test-only RSA key (never used anywhere else) that signs the Access JWTs in these tests.
pub const TEST_KEY_DER: &[u8] = include_bytes!("../data/access-test-key.der");
pub const TEST_KEY_N: &str = "tl3NspTl_6X-JUN7FcoQv4_Gn_HHgDB5U4_cp4DfyLxh18TCc7OLYSsXgk2NSjtm4kl_sOQUX1ad0-D6fIeC9-HhtQrzADP5tionvBkCHXDFRKq-H7HumYRk_492WzmY3WD65MLey16c8eJrU90imayZPzk4zeITH2pQAYzxahxUsd6coBQlg4nnILysy4XkF-uLYFpPT19p4n5Ff26agBLXG0naMmktH4GoXkHfxxiqtGX9HGnXLEB9-qDUSynKb73I3CpdrjCyZfhfI-y3mCSJZZE5fH1VgsUVby8yn-sRd8RoJqbEtKJF5kZIUvsAExuYT26doe9GO-1e3tiE-Q";
pub const AUDIENCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const TEAM_DOMAIN: &str = "acme.cloudflareaccess.com";
pub const PUBLIC_HOST: &str = "agency.example.com";

/// A change to a token's claims.
pub type EditClaims = Box<dyn FnOnce(&mut Value)>;

pub struct TestCerts;

#[async_trait]
impl CertSource for TestCerts {
    async fn certs(&self, _team_domain: &str) -> Result<Value, String> {
        Ok(
            json!({ "keys": [{ "kid": "k1", "kty": "RSA", "alg": "RS256", "n": TEST_KEY_N, "e": "AQAB" }] }),
        )
    }
}

/// A token for `email`, valid around T0 unless `edit` changes the claims.
pub fn access_token(email: &str, edit: impl FnOnce(&mut Value)) -> String {
    let t0 = 1_795_521_600; // 2026-11-24T12:00:00Z
    let mut claims = json!({ "aud": [AUDIENCE], "iss": format!("https://{TEAM_DOMAIN}"), "exp": t0 + 600, "nbf": t0 - 60, "email": email });
    edit(&mut claims);
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("k1".into());
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_rsa_der(TEST_KEY_DER),
    )
    .unwrap()
}

/// alice's node behind Access with owner `alice@example.com`, the app identifier `app_id`, and one
/// request (`r-1`) waiting.
pub async fn behind_access(app_id: Option<&str>) -> Running {
    behind_access_with_runtime(app_id).await.0
}

/// `behind_access`, with the team runtime's memory, for a test that addresses requests.
#[allow(dead_code)]
pub async fn behind_access_with_runtime(
    app_id: Option<&str>,
) -> (
    Running,
    Arc<std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>>,
) {
    let (runtime, memory) = memory_runtime();
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-1".into(), "Ship it?".into()));
    let mut running = node_with("alice", runtime, Some(Arc::new(TestCerts))).await;
    agency_core::orgs::create(&running.node, id("acme"), "Acme".into()).unwrap();
    let access = AccessConfig {
        public_host: PUBLIC_HOST.into(),
        team_domain: TEAM_DOMAIN.into(),
        audience: AUDIENCE.into(),
        owner_email: "alice@example.com".into(),
        app_id: app_id.map(str::to_owned),
    };
    running
        .node
        .update_config(|config| config.access = Some(access))
        .unwrap();
    running.stop().await;
    running.start().await;
    (running, memory)
}
