//! 署名したもの: `{ body, signer, signature }`。署名する文字列は「種類の行 + 決まった順の JSON」。
use crate::canonical::{canonical, canonical_of};
use crate::ids::Id;
use crate::keys::{AccountKey, PublicKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 署名の種類。種類の行があるので、ある種類の署名を別の種類に流用できない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Envelope,
    Org,
    Join,
    Read,
}

impl Kind {
    fn line(self) -> &'static str {
        match self {
            Self::Envelope => "agency/v1/envelope\n",
            Self::Org => "agency/v1/org\n",
            Self::Join => "agency/v1/join\n",
            Self::Read => "agency/v1/read\n",
        }
    }

    pub fn message(self, text: &str) -> Vec<u8> {
        [self.line().as_bytes(), text.as_bytes()].concat()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Signed<T> {
    pub body: T,
    pub signer: Id,
    pub signature: String,
}

impl<T: Serialize + DeserializeOwned> Signed<T> {
    pub fn sign(kind: Kind, body: T, signer: Id, key: &AccountKey) -> serde_json::Result<Self> {
        let signature = key.sign(&kind.message(&canonical_of(&body)?));
        Ok(Self {
            body,
            signer,
            signature,
        })
    }

    /// 本体の決まった順の JSON に対して、署名がこの公開鍵で合うか
    pub fn verify(&self, kind: Kind, key: &PublicKey) -> bool {
        canonical_of(&self.body)
            .map(|text| key.verify(&kind.message(&text), &self.signature))
            .unwrap_or(false)
    }

    /// 署名したもの全体の指紋 (決まった順の JSON の SHA-256 の 16 進、64 文字)。組織の定義の版をつなぐ
    pub fn digest(&self) -> String {
        let text = serde_json::to_value(self)
            .map(|value| canonical(&value))
            .unwrap_or_default();
        Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// 読み出しの署名の文字列 (本文の無い GET)
pub fn read_text(method: &str, path: &str, date: &str, account: &Id) -> String {
    format!("{method}\n{path}\n{date}\n{account}")
}
