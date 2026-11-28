//! The test issuer itself: it is reached over TLS only by a client that trusts its test CA, it serves
//! discovery and a key set, and it says what it was asked.
mod common;

use common::oidc_issuer::{jwk, Answer, Issuer, ISSUER_PATH, ROTATED_KEY_N, TEST_CA_DER};
use reqwest::{redirect::Policy, Certificate, Client};
use serde_json::Value;

fn trusting_client() -> Client {
    Client::builder()
        .add_root_certificate(Certificate::from_der(TEST_CA_DER).unwrap())
        .redirect(Policy::none())
        .build()
        .unwrap()
}

#[tokio::test]
async fn serves_discovery_and_a_key_set_to_a_client_that_trusts_its_ca() {
    let issuer = Issuer::start().await;
    let client = trusting_client();

    let discovery: Value = client
        .get(format!(
            "{}/.well-known/openid-configuration",
            issuer.issuer()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(discovery["issuer"], issuer.issuer());
    assert_eq!(discovery["jwks_uri"], issuer.keys_url());

    let keys: Value = client
        .get(issuer.keys_url())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(keys["keys"][0]["kid"], "k1");
    assert_eq!(
        issuer.asked(),
        [
            format!("{ISSUER_PATH}/.well-known/openid-configuration"),
            format!("{ISSUER_PATH}/keys"),
        ]
    );
    assert_eq!(issuer.times_asked(&format!("{ISSUER_PATH}/keys")), 1);
}

#[tokio::test]
async fn is_not_reachable_by_a_client_that_does_not_trust_its_ca() {
    let issuer = Issuer::start().await;
    let plain = Client::builder().build().unwrap();
    assert!(plain.get(issuer.keys_url()).send().await.is_err());
}

#[tokio::test]
async fn answers_what_it_is_told_to_and_publishes_new_keys() {
    let issuer = Issuer::start().await;
    let client = trusting_client();
    let keys_path = format!("{ISSUER_PATH}/keys");

    issuer.publish(vec![jwk("k2", ROTATED_KEY_N)]);
    let keys: Value = client
        .get(issuer.keys_url())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(keys["keys"][0]["kid"], "k2");

    issuer.answer(&keys_path, Answer::Status(503));
    assert_eq!(
        client.get(issuer.keys_url()).send().await.unwrap().status(),
        503
    );

    issuer.answer(
        &keys_path,
        Answer::Redirect("https://elsewhere.example.test/keys".into()),
    );
    let response = client.get(issuer.keys_url()).send().await.unwrap();
    assert_eq!(
        response.status(),
        302,
        "a client that follows no redirect sees it"
    );
    assert_eq!(
        response.headers()["location"],
        "https://elsewhere.example.test/keys"
    );

    assert_eq!(
        client
            .get(format!("{}/other", issuer.origin()))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}
