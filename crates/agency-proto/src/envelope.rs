//! 決裁の封筒。決めた人のアカウントの鍵で署名し、チームの台が確かめてから当てる。
use crate::clock;
use crate::ids::{EnvelopeId, Id};
use serde::{Deserialize, Serialize};

pub const MAX_TEXT: usize = 4000;
/// 作ってから受け取りを許す長さ (寝ていた台が起きて届けるのを待つ)
pub const MAX_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvelopeKind {
    /// 要求に答える (target は要求の id)
    Answer,
    /// ボードの判断待ちに答える (target は repo#番号)
    Judge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub id: EnvelopeId,
    pub kind: EnvelopeKind,
    pub org: Id,
    pub team: Id,
    pub target: String,
    pub payload: Payload,
    pub account: Id,
    pub at: String,
}

fn is_target(raw: &str) -> bool {
    (1..=120).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_alphanumeric() || b"._#-".contains(&b))
}

impl Envelope {
    /// 形の確かめ (署名の確かめとは別)。台の境目で、当てる前に呼ぶ
    pub fn validate(&self) -> Result<(), String> {
        if !is_target(&self.target) {
            return Err("宛ての形が違う".into());
        }
        if self.payload.text.chars().count() > MAX_TEXT {
            return Err(format!("答えの文は {MAX_TEXT} 文字まで"));
        }
        if let Some(choice) = &self.payload.choice {
            if choice.is_empty() || choice.chars().count() > 20 {
                return Err("選択肢の形が違う".into());
            }
        }
        if self.payload.text.trim().is_empty() && self.payload.choice.is_none() {
            return Err("選択肢か答えの文が要る".into());
        }
        clock::parse(&self.at).map(|_| ()).ok_or_else(|| "時刻の形が違う".into())
    }

    /// 作ってからの長さが受け取りの期限の内か (先の時刻は、時計のずれの 5 分まで)
    pub fn is_fresh(&self, now: time::OffsetDateTime) -> bool {
        clock::parse(&self.at).is_some_and(|at| {
            let age = (now - at).whole_seconds();
            (-300..=MAX_AGE_SECONDS).contains(&age)
        })
    }
}
