//! 送り箱。ほかの台へ届けるもの (決裁の封筒と、組織の定義の新しい版) は、まずここに置き、届いたら消す。
//! 届かなければ間を空けて送り直す (ため置いて送る)。相手が 4xx で断ったら送り直さず、断られた箱に移す。
use crate::node::Node;
use crate::store::{read_all, write_json};
use agency_proto::org::SignedOrg;
use agency_proto::{clock, Envelope, Signed};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "doc", rename_all = "lowercase")]
pub enum ParcelBody {
    Envelope(Box<Signed<Envelope>>),
    Org(Box<SignedOrg>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcel {
    pub id: String,
    /// 届け先の台の頭の URL
    pub to: String,
    pub body: ParcelBody,
    pub queued_at: String,
    pub attempts: u32,
    pub next_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Delivered,
    Refused(String),
    Unreachable(String),
}

const BACKOFF_SECONDS: [i64; 5] = [15, 30, 60, 120, 300];

impl Parcel {
    pub fn new(to: String, body: ParcelBody) -> Self {
        let id = match &body {
            ParcelBody::Envelope(signed) => signed.body.id.as_str().to_owned(),
            ParcelBody::Org(doc) => Sha256::digest(format!("{to}\n{}\n{}", doc.body.org, doc.body.version)).iter().take(16).map(|b| format!("{b:02x}")).collect(),
        };
        let now = clock::format(clock::now());
        Self { id, to, body, queued_at: now.clone(), attempts: 0, next_at: now, last_error: None }
    }

    fn url(&self) -> String {
        match &self.body {
            ParcelBody::Envelope(signed) => format!("{}/fed/v1/teams/{}/{}/envelopes", self.to, signed.body.org, signed.body.team),
            ParcelBody::Org(doc) => format!("{}/fed/v1/orgs/{}/definition", self.to, doc.body.org),
        }
    }

    fn doc(&self) -> serde_json::Value {
        match &self.body {
            ParcelBody::Envelope(signed) => serde_json::to_value(signed),
            ParcelBody::Org(doc) => serde_json::to_value(doc),
        }
        .unwrap_or_default()
    }
}

pub fn enqueue(node: &Node, parcel: &Parcel) -> Result<()> {
    write_json(&node.data.dir("outbox").join(format!("{}.json", parcel.id)), parcel)
}

pub fn pending(node: &Node) -> Vec<Parcel> {
    read_all(&node.data.dir("outbox"))
}

async fn post(node: &Node, parcel: &Parcel) -> Outcome {
    let response = match node.http.post(parcel.url()).json(&parcel.doc()).send().await {
        Ok(response) => response,
        Err(error) => return Outcome::Unreachable(error.to_string()),
    };
    let status = response.status();
    if status.is_success() {
        return Outcome::Delivered;
    }
    let message = response.json::<serde_json::Value>().await.ok().and_then(|body| body["error"].as_str().map(str::to_owned)).unwrap_or_else(|| status.to_string());
    let is_retryable = status.as_u16() == 408 || status.as_u16() == 429 || status.is_server_error();
    if is_retryable {
        Outcome::Unreachable(message)
    } else {
        Outcome::Refused(message)
    }
}

/// 1 つ届け、結果で消すか、断られた箱に移すか、送り直しの時刻を決める
pub async fn deliver(node: &Node, parcel: &Parcel) -> Result<Outcome> {
    let outcome = post(node, parcel).await;
    let path = node.data.dir("outbox").join(format!("{}.json", parcel.id));
    match &outcome {
        Outcome::Delivered => {
            let _ = fs::remove_file(&path);
        }
        Outcome::Refused(message) => {
            write_json(&node.data.dir("refused").join(format!("{}.json", parcel.id)), &Parcel { last_error: Some(message.clone()), ..parcel.clone() })?;
            let _ = fs::remove_file(&path);
        }
        Outcome::Unreachable(message) => {
            let wait = BACKOFF_SECONDS[(parcel.attempts as usize).min(BACKOFF_SECONDS.len() - 1)];
            let next_at = clock::format(clock::now() + time::Duration::seconds(wait));
            write_json(&path, &Parcel { attempts: parcel.attempts + 1, next_at, last_error: Some(message.clone()), ..parcel.clone() })?;
        }
    }
    Ok(outcome)
}

/// 送り直しの時刻が来たものを、古い順に届ける
pub async fn deliver_due(node: &Node) -> Result<Vec<Outcome>> {
    let now = clock::now();
    let mut outcomes = vec![];
    for parcel in pending(node).into_iter().filter(|one| clock::parse(&one.next_at).is_none_or(|at| at <= now)) {
        outcomes.push(deliver(node, &parcel).await?);
    }
    Ok(outcomes)
}
