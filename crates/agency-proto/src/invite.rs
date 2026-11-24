//! 招待のリンクと参加の申し込み。リンクには組織の根の鍵の指紋を入れ、招かれた側が偽の台を見分けられるようにする。
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::org::is_node_origin;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invite {
    pub node: String,
    pub org: Id,
    /// 組織の根の鍵の指紋 (32 文字の 16 進)
    pub fp: String,
    /// 合言葉 (16 バイトの乱数の base64url)
    pub code: String,
}

const SCHEME: &str = "agency:join?";

/// 招待の合言葉。台には SHA-256 だけを残す
pub fn new_code() -> String {
    let mut bytes = [0u8; 16];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn code_digest(code: &str) -> String {
    Sha256::digest(code.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn encode(raw: &str) -> String {
    raw.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

impl Invite {
    pub fn to_link(&self) -> String {
        format!("{SCHEME}node={}&org={}&fp={}&code={}", encode(&self.node), self.org, self.fp, self.code)
    }

    pub fn parse(link: &str) -> Result<Self, String> {
        let query = link.trim().strip_prefix(SCHEME).ok_or("招待のリンクの形が違う (agency:join? で始まる)")?;
        let field = |name: &str| query.split('&').find_map(|pair| pair.strip_prefix(&format!("{name}="))).and_then(decode);
        let node = field("node").filter(|one| is_node_origin(one)).ok_or("台の URL の形が違う")?;
        let org = field("org").and_then(|one| Id::parse(&one).ok()).ok_or("組織の id の形が違う")?;
        let fp = field("fp").filter(|one| one.len() == 32 && one.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("鍵の指紋の形が違う")?;
        let code = field("code").filter(|one| (16..=64).contains(&one.len()) && one.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))).ok_or("合言葉の形が違う")?;
        Ok(Self { node, org, fp, code })
    }
}

/// 参加の申し込み (種類 join)。署名は本体の publicKey で確かめる
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JoinRequest {
    pub org: Id,
    pub code: String,
    pub account: Id,
    pub label: String,
    pub public_key: PublicKey,
    pub at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_round_trips_and_rejects_bad_parts() {
        let invite = Invite { node: "https://node.example:8443".into(), org: Id::parse("acme").unwrap(), fp: "0123456789abcdef0123456789abcdef".into(), code: new_code() };
        assert_eq!(Invite::parse(&invite.to_link()).unwrap(), invite);
        let bad_node = invite.to_link().replace("node=https%3A%2F%2Fnode.example%3A8443", "node=ftp%3A%2F%2Fx");
        assert!(Invite::parse(&bad_node).is_err());
        assert!(Invite::parse("https://example.com/?org=acme").is_err());
    }
}
