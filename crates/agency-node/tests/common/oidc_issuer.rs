//! A test OpenID Connect issuer for the integration tests: a loopback HTTPS server that serves discovery and
//! a key set, and signs RS256 tokens. The node reaches it through its real key fetch (no redirect, 10 s),
//! trusting only the test CA that signed the server's certificate (`oidc-test-ca.der`).
use super::access::{TEST_KEY_DER, TEST_KEY_N};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

/// The test CA that signed the issuer's certificate: what the node's key fetch is told to trust.
pub const TEST_CA_DER: &[u8] = include_bytes!("../data/oidc-test-ca.der");
const LEAF_DER: &[u8] = include_bytes!("../data/oidc-test-leaf.der");
const LEAF_KEY_DER: &[u8] = include_bytes!("../data/oidc-test-leaf-key.der");
/// A second test-only RSA key, for the key rotation tests.
pub const ROTATED_KEY_DER: &[u8] = include_bytes!("../data/oidc-rotated-test-key.der");
pub const ROTATED_KEY_N: &str = "0uhu-BQVwH6JLksSUtm18bu9xF_-Uqiv_rp296XHJ7FgStWabr_nf_zSaAFjUT_8sjP91DuqZ6NWhE6SBUs_yOmqi0awtjPoOCaDr_Bwi67EYFcwgPdb-z5RJccoB6cGYk5JsKKFp4KWe1IQdTAM8W641lnXNJeCwjYvG9NuS3VHR5PXtk5JCcC9Ij_0Yj2ZipPKSi3FC2awtKIL_YRWFugUVTGMa7pgYjQV6S-e12lshlsOEGtuGmzxmvN7LqqV5rAURE0RvvgRsYPnJ7D5K0t_yckl_ewJ0ayaAGMfzBmn8Ke-XmETi19Z23UzRD9kb5oMbZUfZD146zZ5dRAHzQ";

/// The path of the issuer under its origin, so that an issuer with a path is the case that is tested.
pub const ISSUER_PATH: &str = "/realms/acme";

/// What the server answers to one path.
#[derive(Clone)]
pub enum Answer {
    /// 200 with this JSON.
    Json(Value),
    /// This status, with no body.
    Status(u16),
    /// A redirect to this location.
    Redirect(String),
}

#[derive(Default)]
struct State {
    answers: HashMap<String, Answer>,
    asked: Vec<String>,
}

/// The JWK of a test key: `kid`, and the modulus `n`.
pub fn jwk(kid: &str, n: &str) -> Value {
    json!({ "kid": kid, "kty": "RSA", "alg": "RS256", "use": "sig", "n": n, "e": "AQAB" })
}

/// A key set holding the keys of `jwks`.
pub fn key_set(jwks: Vec<Value>) -> Value {
    json!({ "keys": jwks })
}

pub struct Issuer {
    origin: String,
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl Drop for Issuer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Issuer {
    /// An issuer that serves discovery, and the key set `k1` (the access test key), on a loopback port.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let state = Arc::new(Mutex::new(State::default()));
        let issuer = format!("{origin}{ISSUER_PATH}");
        {
            let mut state = state.lock().unwrap();
            state.answers.insert(
                format!("{ISSUER_PATH}/.well-known/openid-configuration"),
                Answer::Json(json!({ "issuer": issuer, "jwks_uri": format!("{issuer}/keys") })),
            );
            state.answers.insert(
                format!("{ISSUER_PATH}/keys"),
                Answer::Json(key_set(vec![jwk("k1", TEST_KEY_N)])),
            );
        }
        let acceptor = acceptor();
        let served = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let (acceptor, state) = (acceptor.clone(), served.clone());
                tokio::spawn(async move {
                    if let Ok(stream) = acceptor.accept(stream).await {
                        let _ = serve(stream, state).await;
                    }
                });
            }
        });
        Self {
            origin,
            state,
            task,
        }
    }

    /// `https://localhost:<port>`, with no path.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The issuer: the origin and `ISSUER_PATH`, as a token's `iss` and the setting's `issuer` say it.
    pub fn issuer(&self) -> String {
        format!("{}{ISSUER_PATH}", self.origin)
    }

    /// Where the key set is, as discovery says it.
    pub fn keys_url(&self) -> String {
        format!("{}/keys", self.issuer())
    }

    /// Answers `path` (under the origin) as `answer` from now on.
    pub fn answer(&self, path: &str, answer: Answer) {
        self.state
            .lock()
            .unwrap()
            .answers
            .insert(path.to_owned(), answer);
    }

    /// Publishes these keys as the issuer's key set.
    pub fn publish(&self, jwks: Vec<Value>) {
        self.answer(&format!("{ISSUER_PATH}/keys"), Answer::Json(key_set(jwks)));
    }

    /// The paths asked of the server so far, in order.
    pub fn asked(&self) -> Vec<String> {
        self.state.lock().unwrap().asked.clone()
    }

    /// How many times `path` was asked for.
    pub fn times_asked(&self, path: &str) -> usize {
        self.asked()
            .iter()
            .filter(|asked| asked.as_str() == path)
            .count()
    }
}

fn acceptor() -> TlsAcceptor {
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(LEAF_DER.to_vec())],
            PrivateKeyDer::try_from(LEAF_KEY_DER.to_vec()).unwrap(),
        )
        .unwrap();
    TlsAcceptor::from(Arc::new(config))
}

/// Reads one request (a GET, no body) and answers it, then closes.
async fn serve<S>(mut stream: S, state: Arc<Mutex<State>>) -> std::io::Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        head.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let answer = {
        let mut state = state.lock().unwrap();
        state.asked.push(path.clone());
        state.answers.get(&path).cloned()
    };
    let response = match answer {
        Some(Answer::Json(body)) => {
            let body = body.to_string();
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
        }
        Some(Answer::Status(status)) => {
            format!("HTTP/1.1 {status} Test\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
        }
        Some(Answer::Redirect(location)) => format!(
            "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        ),
        None => "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_owned(),
    };
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

/// A token for `email` from this issuer, signed by `key` (`TEST_KEY_DER` as `k1`, or `ROTATED_KEY_DER`
/// as `k2`), valid around T0 (2026-11-24T12:00:00Z) unless `edit` changes the claims.
pub fn token(
    issuer: &str,
    audience: &str,
    email: &str,
    kid: &str,
    edit: impl FnOnce(&mut Value),
) -> String {
    let t0 = 1_795_521_600;
    let mut claims = json!({
        "iss": issuer, "aud": audience, "exp": t0 + 600, "nbf": t0 - 60, "iat": t0 - 60,
        "sub": "subject-1", "email": email,
    });
    edit(&mut claims);
    let key = match kid {
        "k2" => ROTATED_KEY_DER,
        _ => TEST_KEY_DER,
    };
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_rsa_der(key),
    )
    .unwrap()
}
