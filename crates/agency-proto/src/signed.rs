//! Signed documents: `{ body, signer, signature }`. The signed text is a kind line followed by canonical JSON.
use crate::canonical::{canonical, canonical_of};
use crate::ids::Id;
use crate::keys::{AccountKey, PublicKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

/// What a signature is for. Because of the kind line, a signature for one kind cannot be reused for another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Envelope,
    Org,
    Join,
    Read,
    /// A read bound to the node it is made for (`read_text_for`): a new signed-text version of `Read`.
    ReadFor,
    Device,
}

impl Kind {
    fn line(self) -> &'static str {
        match self {
            Self::Envelope => "agency/v1/envelope\n",
            Self::Org => "agency/v1/org\n",
            Self::Join => "agency/v1/join\n",
            Self::Read => "agency/v1/read\n",
            Self::ReadFor => "agency/v2/read\n",
            Self::Device => "agency/v1/device\n",
        }
    }

    pub fn message(self, text: &str) -> Vec<u8> {
        [self.line().as_bytes(), text.as_bytes()].concat()
    }
}

/// `signer` is an account id, or a device id for envelopes and device changes signed by a device.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
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

    /// Signs with a key held elsewhere: `sign` gets the signed text and answers the signature (base64),
    /// or why it could not sign.
    pub fn sign_with<E>(
        kind: Kind,
        body: T,
        signer: Id,
        sign: impl FnOnce(&[u8]) -> Result<String, E>,
    ) -> Result<Result<Self, E>, serde_json::Error> {
        let text = kind.message(&canonical_of(&body)?);
        Ok(sign(&text).map(|signature| Self {
            body,
            signer,
            signature,
        }))
    }

    /// Whether the signature over the body's canonical JSON matches this public key.
    pub fn verify(&self, kind: Kind, key: &PublicKey) -> bool {
        canonical_of(&self.body)
            .map(|text| key.verify(&kind.message(&text), &self.signature))
            .unwrap_or(false)
    }

    /// Fingerprint of the whole signed document (hex SHA-256 of its canonical JSON, 64 characters). Links org definition versions.
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

/// The text signed for a signed read (a GET without a body).
pub fn read_text(method: &str, path: &str, date: &str, account: &Id) -> String {
    format!("{method}\n{path}\n{date}\n{account}")
}

/// The text signed for a signed read that names the node it is made for (kind `ReadFor`): the text of
/// `read_text` and a line with `audience`, the id of that node (`client::node_id`). A signature made over one
/// of the two texts is no signature over the other, since the kind lines differ.
pub fn read_text_for(method: &str, path: &str, date: &str, account: &Id, audience: &str) -> String {
    format!("{}\n{audience}", read_text(method, path, date, account))
}

/// Whether `text` is written as a node id: 32 lowercase hex characters (`client::node_id`).
pub fn is_node_id(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
