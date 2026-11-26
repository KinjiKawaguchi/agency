//! A validly signed Access token whose time claims have the wrong type is refused: the node reads `exp`
//! and `nbf` as integers itself, so a string or an object never passes as "no limit".
mod common;

use common::access::{access_token, behind_access, PUBLIC_HOST};
use serde_json::{json, Value};

async fn status_with(edit: impl FnOnce(&mut Value)) -> u16 {
    let running = behind_access(None).await;
    reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header(
            "cf-access-jwt-assertion",
            access_token("alice@example.com", edit),
        )
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn time_claims_of_the_wrong_type_are_refused() {
    assert_eq!(
        status_with(|_| {}).await,
        200,
        "the same token with integer claims"
    );
    let far = 1_795_521_600_i64 + 600;
    let cases: [(&str, Box<dyn FnOnce(&mut Value)>); 7] = [
        (
            "exp as a string",
            Box::new(move |c| c["exp"] = json!(far.to_string())),
        ),
        (
            "exp as an object",
            Box::new(|c| c["exp"] = json!({ "at": 1 })),
        ),
        (
            "exp as a float",
            Box::new(move |c| c["exp"] = json!(far as f64 + 0.5)),
        ),
        ("exp as null", Box::new(|c| c["exp"] = Value::Null)),
        (
            "exp missing",
            Box::new(|c| {
                c.as_object_mut().unwrap().remove("exp");
            }),
        ),
        ("nbf as a string", Box::new(|c| c["nbf"] = json!("0"))),
        ("nbf as an array", Box::new(|c| c["nbf"] = json!([0]))),
    ];
    for (case, edit) in cases {
        assert_eq!(status_with(edit).await, 403, "{case}");
    }
}
