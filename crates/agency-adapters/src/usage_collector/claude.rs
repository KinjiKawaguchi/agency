//! A Claude subscription's usage windows, from the usage endpoint of the Claude Code login.
use super::claude_login::LoginStore;
use super::schedule::Failure;
use super::UsageSource;
use agency_core::ports::Clock;
use async_trait::async_trait;
use reqwest::header::{HeaderValue, AUTHORIZATION, RETRY_AFTER};
use reqwest::StatusCode;
use serde_json::{Map, Value};
use std::sync::Arc;
use std::time::Duration;

pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub struct ClaudeUsage {
    logins: Arc<dyn LoginStore>,
    clock: Arc<dyn Clock>,
    http: reqwest::Client,
    url: String,
}

impl ClaudeUsage {
    pub fn new(
        logins: Arc<dyn LoginStore>,
        clock: Arc<dyn Clock>,
        url: String,
    ) -> reqwest::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(crate::USER_AGENT)
            .build()?;
        Ok(Self {
            logins,
            clock,
            http,
            url,
        })
    }

    /// The access token that lasts longest, if it has not run out.
    async fn token(&self) -> Result<String, Failure> {
        let now_ms = self.clock.now().unix_timestamp() * 1000;
        let logins = self.logins.logins().await;
        let newest = logins.into_iter().max_by_key(|login| login.expires_at_ms);
        match newest {
            None => Err(Failure::NoLogin(self.logins.describe())),
            Some(login) if login.expires_at_ms <= now_ms => Err(Failure::Expired),
            Some(login) => Ok(login.token.expose().to_owned()),
        }
    }
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let seconds = response
        .headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds))
}

/// The two windows with their percent and reset time, and nothing else the answer holds.
fn windows(body: &str) -> Result<Value, Failure> {
    let answer: Value = serde_json::from_str(body).map_err(|_| Failure::Malformed)?;
    let answer = answer.as_object().ok_or(Failure::Malformed)?;
    let names = ["five_hour", "seven_day"];
    if names.iter().all(|name| !answer.contains_key(*name)) {
        return Err(Failure::Malformed);
    }
    let mut kept = Map::new();
    for name in names {
        let Some(window) = answer.get(name).and_then(Value::as_object) else {
            continue;
        };
        let fields = ["utilization", "resets_at"];
        let window: Map<String, Value> = fields
            .iter()
            .filter_map(|field| Some((field.to_string(), window.get(*field)?.clone())))
            .collect();
        kept.insert(name.to_owned(), Value::Object(window));
    }
    Ok(Value::Object(kept))
}

#[async_trait]
impl UsageSource for ClaudeUsage {
    async fn read(&self) -> Result<Value, Failure> {
        let token = self.token().await?;
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| Failure::Malformed)?;
        authorization.set_sensitive(true);
        let response = self
            .http
            .get(&self.url)
            .header(AUTHORIZATION, authorization)
            .header("anthropic-beta", "oauth-2025-04-20")
            .send()
            .await
            .map_err(|error| Failure::Unavailable(error.without_url().to_string()))?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => return Err(Failure::Rejected),
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(Failure::RateLimited(retry_after(&response)))
            }
            status => return Err(Failure::Unavailable(format!("HTTP {}", status.as_u16()))),
        }
        let body = response
            .text()
            .await
            .map_err(|error| Failure::Unavailable(error.without_url().to_string()))?;
        windows(&body)
    }
}
