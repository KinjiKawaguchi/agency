//! The Client API end to end: what a client reads and decides, and who is let in.
mod common;

use agency_core::config::FundingConfig;
use agency_core::views::refresh;
use agency_proto::client::FundingKind;
use agency_proto::AccountKey;
use common::access::{
    access_token, behind_access, EditClaims, AUDIENCE, PUBLIC_HOST, TEAM_DOMAIN, TEST_KEY_N,
};
use common::{
    alice, body, device, get, id, joined_bob, memory_runtime, node_with, post, signed_answer,
    Running, ENVELOPES, TOKEN,
};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

/// A Client API body for an answer to `target`, signed by `key` as a device of the node's account.
fn decision(running: &Running, key: &AccountKey, target: &str) -> Value {
    json!({ "envelope": signed_answer(running, key, target, "a") })
}

#[tokio::test]
async fn reads_and_decides_on_a_local_team() {
    let (alice, memory) = alice().await;
    let key = device(&alice).await;
    let (status, me) = body(get(&alice, "/api/me").send().await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!(me["account"]["id"], "alice");
    assert_eq!(me["orgs"][0]["isRoot"], true);
    assert_eq!(
        me["orgs"][0]["teams"][0],
        json!({ "id": "app", "label": "app", "location": "local", "viewer": "owner" })
    );

    let (status, snapshot) = body(
        get(&alice, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            snapshot["source"]["kind"].as_str(),
            snapshot["viewer"].as_str()
        ),
        (Some("local"), Some("owner"))
    );
    assert_eq!(snapshot["view"]["requests"][0]["id"], "r-1");
    assert_eq!(snapshot["mine"], json!(["r-1"]), "unaddressed: the owner's");
    assert_eq!(snapshot["waitingOnViewer"], json!([]), "always sent");

    let (status, result) = body(
        post(&alice, ENVELOPES, &decision(&alice, &key, "r-1"))
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        (status, result),
        (StatusCode::OK, json!({ "outcome": "applied" }))
    );
    assert_eq!(memory.lock().unwrap().applied[0].by, id("alice"));

    let (status, _) = body(
        post(&alice, ENVELOPES, &decision(&alice, &key, "r-404"))
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 404, "a request the runtime does not know");
    let (status, org) = body(get(&alice, "/api/orgs/acme").send().await.unwrap()).await;
    assert_eq!(
        (status, org["teams"][0]["node"].as_str()),
        (StatusCode::OK, Some(alice.url().as_str()))
    );
    assert_eq!(
        get(&alice, "/api/orgs/other")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn reads_a_replica_and_decides_on_a_remote_team() {
    let (alice, memory) = alice().await;
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("reviewer".into(), id("bob"));
        })
        .unwrap();
    let bob = joined_bob(&alice).await;
    let key = device(&bob).await;
    let (status, _) = body(
        get(&bob, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 503, "no replica before the first read");

    refresh(&bob.node).await.unwrap();
    let (_, me) = body(get(&bob, "/api/me").send().await.unwrap()).await;
    assert_eq!(me["orgs"][0]["teams"][0]["location"], "remote");
    assert_eq!(
        me["orgs"][0]["teams"][0]["viewer"], "reviewer",
        "named by the team's node"
    );
    let (status, snapshot) = body(
        get(&bob, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            snapshot["source"]["kind"].as_str(),
            snapshot["viewer"].as_str()
        ),
        (Some("replica"), Some("reviewer"))
    );
    assert_eq!(
        snapshot["mine"],
        json!([]),
        "an unaddressed request is the team owner's, not the reviewer's"
    );
    assert_eq!(snapshot["source"]["fetchedAt"], "2026-11-24T12:00:00Z");

    let (status, result) = body(
        post(&bob, ENVELOPES, &decision(&bob, &key, "r-1"))
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        (status, result),
        (StatusCode::OK, json!({ "outcome": "delivered" }))
    );
    assert_eq!(
        memory.lock().unwrap().applied[0].by,
        id("bob"),
        "signed by bob's device, applied as bob"
    );
}

#[tokio::test]
async fn writes_need_the_token_and_a_json_body() {
    let (alice, memory) = alice().await;
    let path = ENVELOPES;
    let key = AccountKey::generate();
    let cases = [
        (
            "no token",
            post(&alice, path, &decision(&alice, &key, "r-1")),
            StatusCode::FORBIDDEN,
        ),
        (
            "wrong token",
            post(&alice, path, &decision(&alice, &key, "r-1"))
                .header("x-agency-token", "test-token-0123456789abcdeX"),
            StatusCode::FORBIDDEN,
        ),
        (
            "form body",
            Client::new()
                .post(format!("{}{path}", alice.url()))
                .header("x-agency-token", TOKEN)
                .header("content-type", "text/plain")
                .body(decision(&alice, &key, "r-1").to_string()),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "too large",
            Client::new()
                .post(format!("{}{path}", alice.url()))
                .header("x-agency-token", TOKEN)
                .header("content-type", "application/json")
                .body(format!("{{\"text\":\"{}\"}}", "a".repeat(70 * 1024))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ];
    for (label, request, expected) in cases {
        assert_eq!(request.send().await.unwrap().status(), expected, "{label}");
    }
    assert!(memory.lock().unwrap().applied.is_empty());
}

#[tokio::test]
async fn only_direct_local_requests_are_let_in_without_access() {
    let (alice, _) = alice().await;
    let port = alice.port;
    let with_host = |host: String| get(&alice, "/api/me").header("host", host);
    assert_eq!(
        with_host(format!("localhost:{port}"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        with_host(format!("evil.example:{port}"))
            .send()
            .await
            .unwrap()
            .status(),
        421,
        "DNS rebinding"
    );
    assert_eq!(
        with_host(format!("127.0.0.1:{}", port + 1))
            .send()
            .await
            .unwrap()
            .status(),
        421
    );
    assert_eq!(
        get(&alice, "/api/me")
            .header("cf-ray", "1")
            .send()
            .await
            .unwrap()
            .status(),
        403,
        "through Cloudflare without an Access setting"
    );
    assert_eq!(
        get(&alice, "/")
            .header("cf-connecting-ip", "198.51.100.7")
            .send()
            .await
            .unwrap()
            .status(),
        403,
        "the page too"
    );
    let federation = get(&alice, "/fed/v1/orgs/acme/definition")
        .header("host", "nodes.example")
        .send()
        .await
        .unwrap();
    assert_eq!(
        federation.status(),
        404,
        "federation is checked by signatures, not by host: an unsigned read is refused like an unknown org"
    );
}

#[tokio::test]
async fn serves_the_web_client_with_its_token_and_headers() {
    let (alice, _) = alice().await;
    for path in ["/", "/desk/acme/app"] {
        let response = get(&alice, path).send().await.unwrap();
        assert_eq!(response.status(), 200, "{path}");
        assert!(response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'"));
        assert!(
            response.text().await.unwrap().contains(&format!(
                "<head><meta name=\"agency-token\" content=\"{TOKEN}\" /><title>"
            )),
            "{path}"
        );
    }
    let script = get(&alice, "/assets/app.js").send().await.unwrap();
    assert_eq!(
        script.headers()["content-type"],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        get(&alice, "/api/nothing").send().await.unwrap().status(),
        404
    );
    assert_eq!(
        get(&alice, "/api/me").send().await.unwrap().headers()["cache-control"],
        "no-store"
    );
}

#[tokio::test]
async fn reports_funding_usage_from_the_usage_file() {
    let (alice, _) = alice().await;
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{"five_hour":{"utilization":41.6,"resets_at":"2026-11-24T15:00:00Z"}}"#,
    )
    .unwrap();
    // `readAt` is when the file was written, not when it was asked for.
    let written = "2026-11-24T11:57:00Z";
    file.as_file()
        .set_modified(agency_proto::clock::parse(written).unwrap().into())
        .unwrap();
    let with_file = FundingConfig {
        id: id("max"),
        label: "Max".into(),
        kind: FundingKind::Subscription,
        usage_file: Some(file.path().into()),
        collector: None,
    };
    let without = FundingConfig {
        id: id("api"),
        label: "API".into(),
        kind: FundingKind::ApiKey,
        usage_file: None,
        collector: None,
    };
    alice
        .node
        .update_config(|config| config.funding = vec![with_file, without])
        .unwrap();
    let (_, me) = body(get(&alice, "/api/me").send().await.unwrap()).await;
    let expected = json!([
        { "id": "max", "label": "Max", "kind": "subscription", "windows": [{ "name": "fiveHour", "lengthHours": 5, "utilization": 42, "resetsAt": "2026-11-24T15:00:00Z" }], "readAt": "2026-11-24T11:57:00Z" },
        { "id": "api", "label": "API", "kind": "apiKey", "windows": [] }
    ]);
    assert_eq!(me["funding"], expected);
}

#[tokio::test]
async fn through_access_only_valid_tokens_read_and_only_the_owner_writes() {
    let running = behind_access(None).await;
    let via_access = |token: String| {
        get(&running, "/api/me")
            .header("host", PUBLIC_HOST)
            .header("cf-access-jwt-assertion", token)
    };
    assert_eq!(
        via_access(access_token("alice@example.com", |_| {}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        via_access(access_token("carol@example.com", |_| {}))
            .send()
            .await
            .unwrap()
            .status(),
        200,
        "Access decides who may read"
    );
    let refused: [(&str, EditClaims); 4] = [
        (
            "expired",
            Box::new(|claims| claims["exp"] = json!(1_795_521_600 - 61)),
        ),
        (
            "not valid yet",
            Box::new(|claims| claims["nbf"] = json!(1_795_521_600 + 61)),
        ),
        (
            "other audience",
            Box::new(|claims| claims["aud"] = json!(["b".repeat(64)])),
        ),
        (
            "other issuer",
            Box::new(|claims| claims["iss"] = json!("https://evil.cloudflareaccess.com")),
        ),
    ];
    for (label, edit) in refused {
        assert_eq!(
            via_access(access_token("alice@example.com", edit))
                .send()
                .await
                .unwrap()
                .status(),
            403,
            "{label}"
        );
    }
    let hs256_claims = json!({ "aud": [AUDIENCE], "iss": format!("https://{TEAM_DOMAIN}"), "exp": 1_795_521_600 + 600, "email": "alice@example.com" });
    let hs256_key = jsonwebtoken::EncodingKey::from_secret(TEST_KEY_N.as_bytes());
    let hs256 = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &hs256_claims,
        &hs256_key,
    )
    .unwrap();
    assert_eq!(
        via_access(hs256).send().await.unwrap().status(),
        403,
        "HS256 keyed with the public key"
    );
    let mut forged = access_token("alice@example.com", |_| {});
    forged.replace_range(forged.len() - 4.., "AAAA");
    assert_eq!(
        via_access(forged).send().await.unwrap().status(),
        403,
        "bad signature"
    );
    assert_eq!(
        get(&running, "/api/me")
            .header("cf-ray", "1")
            .send()
            .await
            .unwrap()
            .status(),
        403,
        "no token"
    );

    let write = |email: &str| {
        post(
            &running,
            ENVELOPES,
            &decision(&running, &AccountKey::generate(), "r-1"),
        )
        .header("cf-ray", "1")
        .header("x-agency-token", TOKEN)
        .header("cf-access-jwt-assertion", access_token(email, |_| {}))
    };
    assert_eq!(
        write("carol@example.com").send().await.unwrap().status(),
        403,
        "only the owner decides"
    );
    assert_ne!(
        write("alice@example.com").send().await.unwrap().status(),
        403,
        "the owner is let through to the decision"
    );
}

#[tokio::test]
async fn a_peer_beyond_loopback_is_refused_even_with_a_local_host() {
    use axum::extract::ConnectInfo;
    use tower::ServiceExt;
    let (alice, _) = alice().await;
    let request = |peer: &str| {
        let mut request = axum::http::Request::get("/api/me")
            .header("host", format!("127.0.0.1:{}", alice.port))
            .body(axum::body::Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
        request
    };
    assert_eq!(
        alice
            .app()
            .oneshot(request("127.0.0.1:50000"))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        alice
            .app()
            .oneshot(request("[::1]:50000"))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        alice
            .app()
            .oneshot(request("192.0.2.10:50000"))
            .await
            .unwrap()
            .status(),
        403,
        "a LAN peer that sets Host"
    );
}

#[tokio::test]
async fn orgs_the_account_does_not_belong_to_are_not_found() {
    let (alice, _) = alice().await;
    let (carol_runtime, _) = memory_runtime();
    let carol = node_with("carol", carol_runtime, None).await;
    let copy = alice.node.store().load_org(&id("acme")).unwrap();
    carol.node.store().save_org(&copy).unwrap();
    let (_, me) = body(get(&carol, "/api/me").send().await.unwrap()).await;
    assert_eq!(me["orgs"], json!([]));
    assert_eq!(
        get(&carol, "/api/orgs/acme").send().await.unwrap().status(),
        404
    );
    assert_eq!(
        get(&carol, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let decided = post(
        &carol,
        ENVELOPES,
        &decision(&carol, &AccountKey::generate(), "r-1"),
    )
    .header("x-agency-token", TOKEN)
    .send()
    .await
    .unwrap();
    assert_eq!(decided.status(), 404);
}

#[tokio::test]
async fn the_session_gives_the_write_token_and_the_nodes_time() {
    let (alice, _) = alice().await;
    let response = get(&alice, "/api/session").send().await.unwrap();
    assert_eq!(
        response.headers()["cache-control"],
        "no-store",
        "the token is not kept by caches"
    );
    let (status, session) = body(response).await;
    assert_eq!(status, 200);
    assert_eq!(session, json!({ "writeToken": TOKEN, "now": common::T0 }));

    let token = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    let write = post(&alice, "/api/push/tokens", &token)
        .header("x-agency-token", session["writeToken"].as_str().unwrap());
    assert_eq!(
        write.send().await.unwrap().status(),
        200,
        "the token it gives writes"
    );
}

#[tokio::test]
async fn through_access_the_session_needs_a_sign_in_and_its_token_writes_only_for_the_owner() {
    let running = behind_access(None).await;
    let session = |token: Option<String>| {
        let request = get(&running, "/api/session").header("host", PUBLIC_HOST);
        match token {
            Some(token) => request.header("cf-access-jwt-assertion", token),
            None => request,
        }
    };
    assert_eq!(session(None).send().await.unwrap().status(), 403);
    let carol = access_token("carol@example.com", |_| {});
    let (status, read) = body(session(Some(carol.clone())).send().await.unwrap()).await;
    assert_eq!(status, 200, "any reader Access lets in, as for the page");
    let token = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    let write = |jwt: String| {
        post(&running, "/api/push/tokens", &token)
            .header("host", PUBLIC_HOST)
            .header("cf-access-jwt-assertion", jwt)
            .header("x-agency-token", read["writeToken"].as_str().unwrap())
    };
    assert_eq!(
        write(carol).send().await.unwrap().status(),
        403,
        "not the owner"
    );
    let alice = access_token("alice@example.com", |_| {});
    assert_eq!(write(alice).send().await.unwrap().status(), 200);
}
