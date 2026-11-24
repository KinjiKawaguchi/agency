//! 組織の定義。根の鍵を持つ台が新しい版に署名し、組織のチームのある台へ送り箱で配る。
//! ほかの台は、今の版の次の版か、信じている根の鍵の指紋の版だけを受け取る。
use crate::node::Node;
use crate::outbox::{enqueue, Parcel, ParcelBody};
use crate::store::{read_json, write_json};
use agency_proto::invite::{code_digest, new_code, Invite, JoinRequest};
use agency_proto::org::{verify_genesis, verify_successor, Root, SignedOrg, TeamPlace};
use agency_proto::{clock, Id, Kind, OrgDefinition, Person, Role, Signed};
use anyhow::{bail, Context, Result};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

pub type Refusal = (StatusCode, String);

/// 招待の控え。合言葉そのものは残さず、SHA-256 だけを名前にする
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteRecord {
    pub org: Id,
    pub department: Option<Id>,
    pub role: Role,
    pub expires_at: String,
    pub used_by: Option<Id>,
}

const INVITE_DAYS: i64 = 7;

fn sign(node: &Node, body: OrgDefinition) -> Result<SignedOrg> {
    body.validate().map_err(anyhow::Error::msg)?;
    Ok(Signed::sign(Kind::Org, body, node.account(), &node.key)?)
}

/// この台が組織の根の鍵を持っているか
pub fn is_root(node: &Node, doc: &SignedOrg) -> bool {
    doc.body.root.account == node.account() && doc.body.root.public_key == node.key.public()
}

/// 新しい版を保存し、組織のチームのある台 (この台を除く) と、所属する人の台へ配る
fn publish(node: &Node, doc: &SignedOrg, extra: &[String]) -> Result<()> {
    node.data.save_org(doc)?;
    let me = node.url();
    let mut targets: Vec<String> = doc.body.teams.iter().map(|team| team.node.clone()).chain(extra.iter().cloned()).filter(|url| url != &me).collect();
    targets.sort();
    targets.dedup();
    for to in targets {
        enqueue(node, &Parcel::new(to, ParcelBody::Org(Box::new(doc.clone()))))?;
    }
    Ok(())
}

pub fn create(node: &Node, org: Id, label: String) -> Result<SignedOrg> {
    if node.data.load_org(&org).is_some() {
        bail!("組織 {org} はもうある");
    }
    let config = node.config();
    let me = Person { id: config.account.id.clone(), label: config.account.label.clone(), account: config.account.id.clone(), public_key: node.key.public(), department: None, role: Role::Member };
    let body = OrgDefinition { org: org.clone(), label, version: 1, prev: None, root: Root { account: node.account(), public_key: node.key.public() }, departments: vec![], people: vec![me], teams: vec![], revoked: vec![], at: clock::format(clock::now()) };
    let doc = sign(node, body)?;
    node.data.save_org(&doc)?;
    node.update_config(|config| config.trust(org, node.key.public().fingerprint()))?;
    Ok(doc)
}

/// 根の鍵の台で、組織の定義を書き換えて次の版にし、配る
pub fn amend(node: &Node, org: &Id, edit: impl FnOnce(&mut OrgDefinition) -> Result<()>) -> Result<SignedOrg> {
    let current = node.data.load_org(org).with_context(|| format!("組織 {org} の定義が無い"))?;
    if !is_root(node, &current) {
        bail!("この台は組織 {org} の根の鍵を持っていない (書き換えられるのは根の鍵だけ)");
    }
    let mut body = OrgDefinition::successor_of(&current, clock::format(clock::now()));
    edit(&mut body)?;
    let doc = sign(node, body)?;
    publish(node, &doc, &[])?;
    Ok(doc)
}

pub fn add_team(node: &Node, org: &Id, team: TeamPlace) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if body.team(&team.id).is_some() {
            bail!("チーム {} はもうある", team.id);
        }
        body.teams.push(team);
        Ok(())
    })
}

pub fn invite(node: &Node, org: &Id, department: Option<Id>, role: Role) -> Result<String> {
    let current = node.data.load_org(org).with_context(|| format!("組織 {org} の定義が無い"))?;
    if !is_root(node, &current) {
        bail!("招待を作れるのは、組織 {org} の根の鍵の台だけ");
    }
    let code = new_code();
    let record = InviteRecord { org: org.clone(), department, role, expires_at: clock::format(clock::now() + time::Duration::days(INVITE_DAYS)), used_by: None };
    write_json(&node.data.dir("invites").join(format!("{}.json", code_digest(&code))), &record)?;
    Ok(Invite { node: node.url(), org: org.clone(), fp: current.body.root.public_key.fingerprint(), code }.to_link())
}

/// 有効な招待か (期限の内で、まだ使われていない)
pub fn open_invite(node: &Node, org: &Id, code: &str) -> Option<InviteRecord> {
    let record: InviteRecord = read_json(&node.data.dir("invites").join(format!("{}.json", code_digest(code))))?;
    let is_open = &record.org == org && record.used_by.is_none() && clock::parse(&record.expires_at).is_some_and(|at| at > clock::now());
    is_open.then_some(record)
}

/// 参加の申し込みを受ける (根の鍵の台)。申し込みの署名、招待、重なりを確かめ、その人を書き足した次の版に署名して返す
pub async fn accept_join(node: &Node, org: &Id, signed: Signed<JoinRequest>) -> Result<SignedOrg, Refusal> {
    let _guard = node.writes.lock().await;
    let request = &signed.body;
    if &request.org != org || signed.signer != request.account || !signed.verify(Kind::Join, &request.public_key) {
        return Err((StatusCode::UNAUTHORIZED, "申し込みの署名が合わない".into()));
    }
    let label_ok = (1..=80).contains(&request.label.chars().count());
    let fresh = clock::parse(&request.at).is_some_and(|at| (clock::now() - at).whole_seconds().abs() <= 600);
    if !label_ok || !fresh {
        return Err((StatusCode::BAD_REQUEST, "名前か時刻の形が違う".into()));
    }
    let record = open_invite(node, org, &request.code).ok_or((StatusCode::FORBIDDEN, "招待が無いか、期限切れか、使われた".into()))?;
    let current = node.data.load_org(org).ok_or((StatusCode::NOT_FOUND, "組織が見つからない".into()))?;
    if current.body.people.iter().any(|one| one.account == request.account || one.id == request.account) {
        return Err((StatusCode::CONFLICT, "そのアカウントはもう組織にいる".into()));
    }
    let person = Person { id: request.account.clone(), label: request.label.clone(), account: request.account.clone(), public_key: request.public_key.clone(), department: record.department.clone(), role: record.role };
    let mut body = OrgDefinition::successor_of(&current, clock::format(clock::now()));
    body.people.push(person);
    let doc = sign(node, body).map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    publish(node, &doc, &[]).map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let used = InviteRecord { used_by: Some(request.account.clone()), ..record };
    write_json(&node.data.dir("invites").join(format!("{}.json", code_digest(&request.code))), &used).map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(doc)
}

/// ほかの台から届いた組織の定義を受け取る。今の版があれば次の版だけ、無ければ信じる根の鍵の指紋の版だけ
pub async fn receive(node: &Node, doc: SignedOrg) -> Result<(), Refusal> {
    let _guard = node.writes.lock().await;
    let org = doc.body.org.clone();
    match node.data.load_org(&org) {
        Some(current) if current.body.version >= doc.body.version => {
            let is_same = current.body.version == doc.body.version && current.digest() == doc.digest();
            if is_same {
                Ok(())
            } else {
                Err((StatusCode::CONFLICT, format!("{org} は {} 版を持っている", current.body.version)))
            }
        }
        Some(current) => {
            verify_successor(&current, &doc).map_err(|message| (StatusCode::BAD_REQUEST, message))?;
            node.data.save_org(&doc).map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
        }
        None => {
            let trusted = node.config().trusted_fingerprint(&org).map(str::to_owned).ok_or((StatusCode::NOT_FOUND, "信じていない組織".into()))?;
            let root = &doc.body.root;
            let is_trusted = root.public_key.fingerprint() == trusted && doc.signer == root.account && doc.verify(Kind::Org, &root.public_key);
            if !is_trusted {
                return Err((StatusCode::UNAUTHORIZED, "信じている根の鍵の署名ではない".into()));
            }
            if doc.body.version == 1 {
                verify_genesis(&doc).map_err(|message| (StatusCode::BAD_REQUEST, message))?;
            }
            doc.body.validate().map_err(|message| (StatusCode::BAD_REQUEST, message))?;
            node.data.save_org(&doc).map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
        }
    }
}
