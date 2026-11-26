//! Other nodes over HTTP (`/fed/v1`). Signed reads travel in the `x-agency-*` headers.
use agency_core::ports::{DefinitionAccess, PeerError, Peers};
use agency_core::read_auth::ReadAuth;
use agency_proto::evaluation::Evaluation;
use agency_proto::org::{SignedOrg, Versions};
use agency_proto::view::FederatedView;
use agency_proto::{DeviceChange, Envelope, JoinRequest, Signed, TokenUsage};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

pub const ACCOUNT: &str = "x-agency-account";
pub const DATE: &str = "x-agency-date";
pub const SIGNATURE: &str = "x-agency-signature";

pub fn read_headers(auth: &ReadAuth) -> [(&'static str, String); 3] {
    [
        (ACCOUNT, auth.account.clone()),
        (DATE, auth.date.clone()),
        (SIGNATURE, auth.signature.clone()),
    ]
}

pub struct HttpPeers {
    http: reqwest::Client,
}

impl HttpPeers {
    pub fn new() -> reqwest::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .user_agent(crate::USER_AGENT)
                // Others choose where this node sends (team places, invite links, team domains); a
                // redirect would send its requests, and their headers, somewhere else.
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(8))
                .build()?,
        })
    }
}

fn unreachable(error: impl std::fmt::Display) -> PeerError {
    PeerError::Unreachable(error.to_string())
}

/// The most this node reads of another node's answer. Another node is chosen by others (an org's root
/// places teams at any origin), so its answers are bounded before they are parsed or kept.
pub const MAX_ANSWER_BYTES: usize = 2 * 1024 * 1024;

/// The answer's body, refused once it is longer than `MAX_ANSWER_BYTES`.
async fn capped_body(mut response: reqwest::Response) -> Result<Vec<u8>, PeerError> {
    let too_long = || {
        unreachable(format!(
            "the answer is longer than {MAX_ANSWER_BYTES} bytes"
        ))
    };
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ANSWER_BYTES as u64)
    {
        return Err(too_long());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(unreachable)? {
        if body.len() + chunk.len() > MAX_ANSWER_BYTES {
            return Err(too_long());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A 2xx answer parsed as `T`; otherwise the status and the answer's `error` text.
async fn answer<T: DeserializeOwned>(
    sent: reqwest::Result<reqwest::Response>,
) -> Result<T, PeerError> {
    let response = sent.map_err(unreachable)?;
    let status = response.status();
    let bytes = capped_body(response).await?;
    if status.is_success() {
        return serde_json::from_slice(&bytes)
            .map_err(|error| unreachable(format!("unreadable answer ({status}): {error}")));
    }
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let message = body["error"]
        .as_str()
        .map_or_else(|| status.to_string(), str::to_owned);
    Err(PeerError::Status {
        status: status.as_u16(),
        message,
    })
}

impl HttpPeers {
    async fn post<B: Serialize + Sync, T: DeserializeOwned>(
        &self,
        node: &str,
        path: &str,
        body: &B,
    ) -> Result<T, PeerError> {
        answer(
            self.http
                .post(format!("{node}{path}"))
                .json(body)
                .send()
                .await,
        )
        .await
    }

    async fn get_signed<T: DeserializeOwned>(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<T, PeerError> {
        let mut request = self.http.get(format!("{node}{path}"));
        for (name, value) in read_headers(auth) {
            request = request.header(name, value);
        }
        answer(request.send().await).await
    }
}

#[async_trait]
impl Peers for HttpPeers {
    async fn get_view(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<FederatedView, PeerError> {
        self.get_signed(node, path, auth).await
    }

    async fn get_usage(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<TokenUsage, PeerError> {
        self.get_signed(node, path, auth).await
    }

    async fn get_evaluation(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Evaluation, PeerError> {
        self.get_signed(node, path, auth).await
    }

    async fn get_definition(
        &self,
        node: &str,
        path: &str,
        access: &DefinitionAccess,
    ) -> Result<SignedOrg, PeerError> {
        match access {
            DefinitionAccess::Signed(auth) => self.get_signed(node, path, auth).await,
            DefinitionAccess::InviteCode(code) => {
                answer(
                    self.http
                        .get(format!("{node}{path}"))
                        .query(&[("code", code)])
                        .send()
                        .await,
                )
                .await
            }
        }
    }

    async fn get_versions(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Versions, PeerError> {
        self.get_signed(node, path, auth).await
    }

    async fn post_envelope(
        &self,
        node: &str,
        path: &str,
        envelope: &Signed<Envelope>,
    ) -> Result<(), PeerError> {
        let _: serde_json::Value = self.post(node, path, envelope).await?;
        Ok(())
    }

    async fn post_definition(
        &self,
        node: &str,
        path: &str,
        doc: &SignedOrg,
    ) -> Result<(), PeerError> {
        let _: serde_json::Value = self.post(node, path, doc).await?;
        Ok(())
    }

    async fn post_device_change(
        &self,
        node: &str,
        path: &str,
        change: &Signed<DeviceChange>,
    ) -> Result<SignedOrg, PeerError> {
        self.post(node, path, change).await
    }

    async fn post_join(
        &self,
        node: &str,
        path: &str,
        request: &Signed<JoinRequest>,
    ) -> Result<SignedOrg, PeerError> {
        self.post(node, path, request).await
    }
}
