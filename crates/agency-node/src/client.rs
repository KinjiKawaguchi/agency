//! ほかの台へ出ていくもの: 写しの読み直し、決裁、参加。
use crate::auth::signed_headers;
use crate::node::Node;
use crate::outbox::{deliver, enqueue, Outcome, Parcel, ParcelBody};
use crate::store::{read_json, write_json};
use agency_proto::invite::Invite;
use agency_proto::org::{verify_successor, SignedOrg};
use agency_proto::{
    clock, Envelope, EnvelopeId, EnvelopeKind, Id, JoinRequest, Kind, OrgDefinition, Payload,
    RuntimeDecision, Signed, TeamView,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A replica of a team on another node. When a read fails, the previous view stays and only the error is recorded.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replica {
    pub fetched_at: Option<String>,
    pub tried_at: String,
    pub error: Option<String>,
    pub view: Option<TeamView>,
}

async fn signed_get(node: &Node, base: &str, path: &str) -> Result<Value, String> {
    let mut request = node.http.get(format!("{base}{path}"));
    for (name, value) in signed_headers(&node.key, &node.account(), "GET", path) {
        request = request.header(name, value);
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|_| format!("答えが JSON でない ({status})"))?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(body["error"]
            .as_str()
            .map_or_else(|| status.to_string(), str::to_owned))
    }
}

/// Another node's view is kept only if it parses as a TeamView of the team that was asked for.
fn parse_view(value: Value, org: &Id, team: &Id) -> Result<TeamView, String> {
    let view: TeamView =
        serde_json::from_value(value).map_err(|error| format!("not a TeamView: {error}"))?;
    if &view.org != org || &view.team != team {
        return Err(format!(
            "the node answered with {}/{} instead of {org}/{team}",
            view.org, view.team
        ));
    }
    Ok(view)
}

pub fn replica_path(node: &Node, org: &Id, team: &Id) -> std::path::PathBuf {
    node.data
        .dir("replicas")
        .join(org.as_str())
        .join(format!("{team}.json"))
}

/// 所属する組織の、ほかの台にあるチームの写しを読み直す。組織の定義も、チームの台から新しい版を受け取る
pub async fn refresh(node: &Node) -> Result<()> {
    let me = node.url();
    let account = node.account();
    for doc in node
        .data
        .orgs()
        .into_iter()
        .filter(|doc| doc.body.member(&account).is_some())
    {
        let org = doc.body.org.clone();
        for place in doc.body.teams.iter().filter(|place| place.node != me) {
            let path = format!("/fed/v1/teams/{org}/{}/view", place.id);
            let file = replica_path(node, &org, &place.id);
            let previous: Replica = read_json(&file).unwrap_or_default();
            let now = clock::format(clock::now());
            let fetched = signed_get(node, &place.node, &path)
                .await
                .and_then(|value| parse_view(value, &org, &place.id));
            let next = match fetched {
                Ok(view) => Replica {
                    fetched_at: Some(now.clone()),
                    tried_at: now,
                    error: None,
                    view: Some(view),
                },
                Err(error) => Replica {
                    tried_at: now,
                    error: Some(error),
                    ..previous
                },
            };
            write_json(&file, &next)?;
        }
        if let Some(place) = doc.body.teams.iter().find(|place| place.node != me) {
            if let Ok(value) =
                signed_get(node, &place.node, &format!("/fed/v1/orgs/{org}/definition")).await
            {
                if let Ok(newer) = serde_json::from_value::<SignedOrg>(value) {
                    let _ = crate::orgs::receive(node, newer).await;
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decided {
    /// この台のチームに当てた
    Applied,
    /// ほかの台に届いた
    Delivered,
    /// 届かなかったので送り箱にため置いた
    Queued(String),
    Refused(String),
}

/// 決裁する。この台のチームならハーネスに当て、ほかの台のチームなら署名した封筒を届ける (届かなければため置く)
pub async fn decide(
    node: &Node,
    org: &Id,
    team: &Id,
    kind: EnvelopeKind,
    target: String,
    payload: Payload,
) -> Result<Decided> {
    let doc = node
        .data
        .load_org(org)
        .with_context(|| format!("組織 {org} の定義が無い"))?;
    let person = doc
        .body
        .member(&node.account())
        .context("この組織に所属していない")?
        .clone();
    let place = doc
        .body
        .team(team)
        .with_context(|| format!("チーム {team} が組織にない"))?
        .clone();
    let envelope = Envelope {
        id: EnvelopeId::random(),
        kind,
        org: org.clone(),
        team: team.clone(),
        target,
        payload,
        account: node.account(),
        at: clock::format(clock::now()),
    };
    envelope.validate().map_err(anyhow::Error::msg)?;
    if let Some(hosted) = node
        .config()
        .hosted(org, team)
        .filter(|_| place.node == node.url())
    {
        let decision = RuntimeDecision {
            kind,
            target: envelope.target,
            by: person.id,
            payload: envelope.payload,
        };
        node.runtimes
            .decide(hosted, decision)
            .await
            .map_err(|error| {
                anyhow::anyhow!(
                    "the team runtime refused ({}): {}",
                    error.status,
                    error.message
                )
            })?;
        return Ok(Decided::Applied);
    }
    let signed = Signed::sign(Kind::Envelope, envelope, node.account(), &node.key)?;
    let parcel = Parcel::new(place.node, ParcelBody::Envelope(Box::new(signed)));
    enqueue(node, &parcel)?;
    Ok(match deliver(node, &parcel).await? {
        Outcome::Delivered => Decided::Delivered,
        Outcome::Unreachable(message) => Decided::Queued(message),
        Outcome::Refused(message) => Decided::Refused(message),
    })
}

/// 招待のリンクから参加する。組織の根の鍵の指紋がリンクと合うかを確かめてから、申し込む
pub async fn join(
    node: &Node,
    link: &str,
    confirm: impl FnOnce(&OrgDefinition) -> bool,
) -> Result<SignedOrg> {
    let invite = Invite::parse(link).map_err(anyhow::Error::msg)?;
    let url = format!("{}/fed/v1/orgs/{}/definition", invite.node, invite.org);
    let current: SignedOrg = node
        .http
        .get(&url)
        .query(&[("code", &invite.code)])
        .send()
        .await?
        .error_for_status()
        .context("招待の台が組織の定義を返さなかった")?
        .json()
        .await?;
    let root = &current.body.root;
    let is_genuine = current.body.org == invite.org
        && root.public_key.fingerprint() == invite.fp
        && current.signer == root.account
        && current.verify(Kind::Org, &root.public_key);
    if !is_genuine {
        bail!("組織の根の鍵がリンクの指紋と合わない (偽の台かもしれない)。参加しない");
    }
    if !confirm(&current.body) {
        bail!("参加をやめた");
    }
    let config = node.config();
    let request = JoinRequest {
        org: invite.org.clone(),
        code: invite.code.clone(),
        account: config.account.id.clone(),
        label: config.account.label.clone(),
        public_key: node.key.public(),
        at: clock::format(clock::now()),
    };
    let signed = Signed::sign(Kind::Join, request, config.account.id.clone(), &node.key)?;
    let response = node
        .http
        .post(format!("{}/fed/v1/orgs/{}/join", invite.node, invite.org))
        .json(&signed)
        .send()
        .await?;
    if !response.status().is_success() {
        let body: Value = response.json().await.unwrap_or_default();
        bail!(
            "参加を断られた: {}",
            body["error"].as_str().unwrap_or("理由不明")
        );
    }
    let joined: SignedOrg = response.json().await?;
    verify_successor(&current, &joined).map_err(anyhow::Error::msg)?;
    if joined.body.member(&config.account.id).is_none() {
        bail!("返ってきた組織の定義に自分がいない");
    }
    node.data.save_org(&joined)?;
    node.update_config(|config| config.trust(invite.org.clone(), invite.fp.clone()))?;
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../../testdata/teamview.sample.json");

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn replicas_keep_only_a_teamview_of_the_team_asked_for() {
        let sample: Value = serde_json::from_str(SAMPLE).unwrap();
        assert!(parse_view(sample.clone(), &id("acme"), &id("app")).is_ok());
        assert!(parse_view(sample.clone(), &id("acme"), &id("other")).is_err());
        assert!(parse_view(sample, &id("other"), &id("app")).is_err());
        assert!(parse_view(serde_json::json!({ "inbox": {} }), &id("acme"), &id("app")).is_err());
    }
}
