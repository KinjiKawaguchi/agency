//! Builders of the vectors of a signed read and of a view's tag.
#![allow(dead_code)]
use super::*;

pub const READ_PATH: &str = "/fed/v1/teams/acme/app/view";
pub const READ_DATE: &str = "2026-11-24T12:00:00.123Z";
pub const READ_AUDIENCE: &str = "02ae509c6b20999646d3c600687ee1bf";
pub const OTHER_AUDIENCE: &str = "0881dc11d90faa5bd241e84dd481c251";

/// A signed read as the account key (seed 7) makes it for a node, in the two texts a node reads: the one that
/// names no node (`agency/v1/read`) and the one bound to the node it is made for (`agency/v2/read`).
pub fn signed_read(key: &AccountKey) -> serde_json::Value {
    let alice = id("alice");
    let unbound = read_text("GET", READ_PATH, READ_DATE, &alice);
    let bound = read_text_for("GET", READ_PATH, READ_DATE, &alice, READ_AUDIENCE);
    serde_json::json!({
        "audience": READ_AUDIENCE,
        "unbound": { "text": unbound, "signature": key.sign(&Kind::Read.message(&unbound)) },
        "bound": { "text": bound, "signature": key.sign(&Kind::ReadFor.message(&bound)) },
    })
}

/// The sample view of `testdata/teamview.sample.json` as a reader named `owner` is answered it.
pub fn sample_answer() -> FederatedView {
    let text = include_str!("../../../../testdata/teamview.sample.json");
    FederatedView {
        view: serde_json::from_str(text).unwrap(),
        viewer: Some("owner".into()),
        answers_for: vec![],
    }
}

/// The tag of a team's view answer: the SHA-256 of its canonical JSON without the view's `generatedAt`.
pub fn view_tag_vector() -> serde_json::Value {
    let answer = sample_answer();
    serde_json::json!({ "answer": answer, "tag": answer.tag() })
}
