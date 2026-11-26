//! Web Push through the Client API: a node without it has no routes for it, one with it gives its VAPID
//! key and keeps a subscription for each browser, whose endpoint must be an allowed push service, and it
//! notifies browsers as it does iPhones (the same kinds, the same once-only rule, the same hidden titles).
mod common;

use agency_core::push::notify_waiting;
use agency_proto::client::VapidKey;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use common::{alice, body, get, id, post, Running, TOKEN};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use reqwest::StatusCode;
use serde_json::{json, Value};

const P256DH: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
const ENDPOINT: &str =
    "https://fcm.googleapis.com/fcm/send/dQw4w9WgXcQ:APA91bHun4MxP5egoKMwt2KZFBaFUH";

fn with_web_push(running: &Running) {
    running.notifier.0.lock().unwrap().web_key = Some(VapidKey {
        public_key: "BPublic".into(),
        key_id: "key-one".into(),
    });
}

fn subscription(endpoint: &str) -> Value {
    json!({ "endpoint": endpoint, "p256dh": P256DH, "auth": AUTH, "label": "Firefox on a laptop" })
}

async fn register(running: &Running, input: &Value) -> (StatusCode, Value) {
    let request = post(running, "/api/push/subscriptions", input).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await
}

fn delete(running: &Running, path: &str) -> reqwest::RequestBuilder {
    reqwest::Client::new().delete(format!("{}{path}", running.url()))
}

async fn list(running: &Running) -> Value {
    body(
        get(running, "/api/push/subscriptions")
            .send()
            .await
            .unwrap(),
    )
    .await
    .1["subscriptions"]
        .clone()
}

fn sent_to_browsers(running: &Running) -> Vec<String> {
    let state = running.notifier.0.lock().unwrap();
    state.sent_web.iter().map(|(id, _)| id.clone()).collect()
}

#[tokio::test]
async fn a_node_without_web_push_has_none_of_its_routes() {
    let (alice, _) = alice().await;
    for path in ["/api/push/vapid", "/api/push/subscriptions"] {
        assert_eq!(
            get(&alice, path).send().await.unwrap().status(),
            404,
            "{path}"
        );
    }
    assert_eq!(register(&alice, &subscription(ENDPOINT)).await.0, 404);
}

#[tokio::test]
async fn the_owner_registers_lists_and_removes_a_subscription() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let key = body(get(&alice, "/api/push/vapid").send().await.unwrap()).await;
    assert_eq!(
        key,
        (
            StatusCode::OK,
            json!({ "publicKey": "BPublic", "keyId": "key-one" })
        )
    );

    let (status, registered) = register(&alice, &subscription(ENDPOINT)).await;
    assert_eq!(status, 200, "{registered}");
    assert_eq!(registered["keyId"], "key-one");
    assert_eq!(registered["events"], json!(["request"]));
    assert_eq!(
        registered["endpoint"], "https://fcm.googleapis.com/…KZFBaFUH",
        "the host and the end of the path"
    );
    assert!(
        !registered.to_string().contains(P256DH) && !registered.to_string().contains(AUTH),
        "keys are never given back"
    );

    let mut again = subscription(ENDPOINT);
    again["label"] = json!("Renamed");
    again["events"] = json!(["decision", "request"]);
    let (_, updated) = register(&alice, &again).await;
    assert_eq!(
        updated["id"], registered["id"],
        "one entry for each endpoint"
    );
    let listed = list(&alice).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(
        (&listed[0]["label"], &listed[0]["events"]),
        (&json!("Renamed"), &json!(["request", "decision"]))
    );

    let id = registered["id"].as_str().unwrap();
    let removal = |id: &str| {
        let request = delete(&alice, &format!("/api/push/subscriptions/{id}"));
        async move {
            request
                .header("x-agency-token", TOKEN)
                .send()
                .await
                .unwrap()
                .status()
        }
    };
    assert_eq!(removal(id).await, 204);
    assert_eq!(removal(id).await, 204, "already gone");
    assert_eq!(list(&alice).await, json!([]));
}

#[tokio::test]
async fn an_endpoint_that_is_not_an_allowed_push_service_is_refused() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    for endpoint in [
        "http://fcm.googleapis.com/fcm/send/x",
        "https://evil.example/fcm/send/x",
        "https://169.254.169.254/latest/meta-data",
        "https://127.0.0.1/x",
        "https://localhost/x",
        "https://fcm.googleapis.com@evil.example/x",
        "https://fcm.googleapis.com:8443/x",
        "https://a.push.apple.com:8443/x",
        "https://user@a.push.apple.com/x",
        "https://a.push.apple.com@evil.example/x",
        "https://evil.example\\@a.push.apple.com/x",
        "https://fcm.googleapis.com/x#frag",
        "https://fcm.googleapis.com",
        "https://notapple.com/x",
        "https://push.apple.com/x",
        "https://evilpush.apple.com.evil.example/x",
        "file:///etc/passwd",
        &format!("https://fcm.googleapis.com/{}", "a".repeat(500)),
    ] {
        let (status, answer) = register(&alice, &subscription(endpoint)).await;
        assert_eq!(status, 400, "{endpoint}: {answer}");
    }
    for endpoint in [
        "https://web.push.apple.com/QOk",
        "https://updates.push.services.mozilla.com/wpush/v2/gAAA",
        "https://wns2-par02p.notify.windows.com/w/?token=x",
    ] {
        assert_eq!(
            register(&alice, &subscription(endpoint)).await.0,
            200,
            "{endpoint}"
        );
    }
    alice
        .node
        .update_config(|config| {
            config.web_push = Some(agency_core::config::WebPushConfig {
                subject: None,
                extra_hosts: vec![
                    "push.acme.example".into(),
                    "*.edge.acme.example".into(),
                    "10.0.0.1".into(),
                    "0x7f.1".into(),
                    "2130706433".into(),
                ],
            });
        })
        .unwrap();
    for (endpoint, status) in [
        ("https://push.acme.example/x", 200),
        ("https://a.edge.acme.example/x", 200),
        ("https://edge.acme.example/x", 400),
        ("https://10.0.0.1/x", 400),
        ("https://0x7f.1/x", 400),
        ("https://2130706433/x", 400),
    ] {
        assert_eq!(
            register(&alice, &subscription(endpoint)).await.0,
            status,
            "{endpoint}"
        );
    }
}

#[tokio::test]
async fn keys_labels_and_the_number_of_subscriptions_are_checked() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let bad = |field: &str, value: Value| {
        let mut input = subscription(ENDPOINT);
        input[field] = value;
        input
    };
    for input in [
        bad("p256dh", json!("short")),
        bad("p256dh", json!(format!("{}=", &P256DH[..86]))),
        bad("p256dh", json!(B64.encode([4u8; 65]))),
        bad(
            "p256dh",
            json!(B64.encode(
                <p256::PublicKey>::from_sec1_bytes(&B64.decode(P256DH).unwrap())
                    .unwrap()
                    .to_encoded_point(true)
                    .as_bytes()
            )),
        ),
        bad("auth", json!(&AUTH[..21])),
        bad("auth", json!("!".repeat(22))),
        bad("label", json!("")),
        bad("label", json!("x".repeat(81))),
        bad("events", json!(["telegram"])),
    ] {
        let (status, answer) = register(&alice, &input).await;
        assert_eq!(status, 400, "{input}: {answer}");
    }
    let mut extra = subscription(ENDPOINT);
    extra["unknown"] = json!(1);
    assert_eq!(register(&alice, &extra).await.0, 400, "no unknown field");
    for n in 0..22 {
        let endpoint = format!("https://fcm.googleapis.com/fcm/send/{n}");
        assert_eq!(register(&alice, &subscription(&endpoint)).await.0, 200);
    }
    let listed = list(&alice).await;
    assert_eq!(
        listed.as_array().unwrap().len(),
        20,
        "the oldest are dropped"
    );
    assert!(!listed.to_string().contains("send/0\""));
}

#[tokio::test]
async fn a_write_needs_the_write_token() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let (status, _) = body(
        post(&alice, "/api/push/subscriptions", &subscription(ENDPOINT))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 403);
    let (_, registered) = register(&alice, &subscription(ENDPOINT)).await;
    let id = registered["id"].as_str().unwrap();
    let status = delete(&alice, &format!("/api/push/subscriptions/{id}"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 403);
    assert_eq!(list(&alice).await.as_array().unwrap().len(), 1);
}

fn add_request(
    memory: &std::sync::Arc<std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>>,
    id: &str,
) {
    memory
        .lock()
        .unwrap()
        .requests
        .push((id.into(), format!("title of {id}")));
}

#[tokio::test]
async fn browsers_are_notified_once_for_the_kinds_they_want_and_a_gone_one_is_removed() {
    let (alice, memory) = alice().await;
    with_web_push(&alice);
    let (_, wants) = register(&alice, &subscription(ENDPOINT)).await;
    let mut other = subscription("https://updates.push.services.mozilla.com/wpush/v2/x");
    other["events"] = json!([]);
    let (_, none) = register(&alice, &other).await;
    notify_waiting(&alice.node).await.unwrap();

    add_request(&memory, "r-2");
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 1);
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "once per request"
    );
    assert_eq!(
        sent_to_browsers(&alice),
        vec![wants["id"].as_str().unwrap().to_owned()],
        "not the one that wants no kind"
    );

    alice.notifier.0.lock().unwrap().failing = true;
    add_request(&memory, "r-3");
    let failed = notify_waiting(&alice.node).await.unwrap();
    assert_eq!((failed.sent, failed.failures.len()), (0, 1));
    alice.notifier.0.lock().unwrap().failing = false;
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        1,
        "tried again"
    );

    alice
        .notifier
        .0
        .lock()
        .unwrap()
        .gone
        .push(wants["id"].as_str().unwrap().to_owned());
    add_request(&memory, "r-4");
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        1,
        "all that wanted it are gone: delivered"
    );
    let left = list(&alice).await;
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["id"], none["id"]);
}

#[tokio::test]
async fn one_token_one_browser_and_either_taking_it_counts() {
    let (alice, memory) = alice().await;
    with_web_push(&alice);
    register(&alice, &subscription(ENDPOINT)).await;
    let token = json!({ "token": "ab".repeat(32), "environment": "sandbox", "label": "iPhone" });
    let request = post(&alice, "/api/push/tokens", &token).header("x-agency-token", TOKEN);
    assert_eq!(request.send().await.unwrap().status(), 200);
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2");
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 1);
    let state = alice.notifier.0.lock().unwrap();
    assert_eq!(
        (state.sent.len(), state.sent_web.len()),
        (1, 1),
        "each destination once"
    );
}

#[tokio::test]
async fn a_subscription_made_with_another_key_is_not_sent_to_and_hidden_titles_reach_browsers() {
    let (alice, memory) = alice().await;
    with_web_push(&alice);
    register(&alice, &subscription(ENDPOINT)).await;
    notify_waiting(&alice.node).await.unwrap();

    alice.notifier.0.lock().unwrap().web_key = Some(VapidKey {
        public_key: "BNew".into(),
        key_id: "key-two".into(),
    });
    add_request(&memory, "r-2");
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        1,
        "nobody to tell is delivered"
    );
    assert!(
        sent_to_browsers(&alice).is_empty(),
        "its push service would refuse the old key's subscription"
    );
    assert_eq!(
        list(&alice).await[0]["keyId"],
        "key-one",
        "kept, and the screen says to subscribe again"
    );

    let (_, again) = register(&alice, &subscription(ENDPOINT)).await;
    assert_eq!(again["keyId"], "key-two");
    alice
        .node
        .update_config(|config| config.hide_titles = true)
        .unwrap();
    add_request(&memory, "r-3");
    notify_waiting(&alice.node).await.unwrap();
    let state = alice.notifier.0.lock().unwrap();
    let agency_core::push::NoticeKind::Request {
        team_label, title, ..
    } = &state.sent_web[0].1.kind
    else {
        panic!("a request");
    };
    assert_eq!(
        (team_label, title),
        (&None, &None),
        "the node-wide setting hides titles for browsers too"
    );
}

/// A node opened from a configuration as `agency` opens it (nothing stands in for its notifier), in a
/// directory that outlives the call.
fn opened_with(
    dir: &std::path::Path,
    web_push: Option<agency_core::config::WebPushConfig>,
) -> std::sync::Arc<agency_core::Node> {
    use agency_adapters::files::DataDir;
    use agency_core::config::{Account, NodeConfig};
    use agency_core::ports::ConfigStore;
    let data = DataDir::new(dir.join("node"));
    if !dir.join("node").exists() {
        data.create_key().unwrap();
    }
    let account = Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    let mut config = NodeConfig::new("http://127.0.0.1:4400", "127.0.0.1:4400", account, "scrum");
    config.web_push = web_push;
    data.save_config(&config).unwrap();
    agency_node::open(data, agency_node::Overrides::default())
        .unwrap()
        .node
}

#[test]
fn a_node_makes_its_vapid_key_once_and_has_no_web_push_with_a_list_that_is_not_names() {
    use agency_core::config::WebPushConfig;
    let dir = tempfile::tempdir().unwrap();
    assert!(agency_core::web_push::vapid_key(&opened_with(dir.path(), None)).is_none());
    assert!(!dir.path().join("node/web-push-vapid.key").exists());

    let first =
        agency_core::web_push::vapid_key(&opened_with(dir.path(), Some(WebPushConfig::default())))
            .unwrap();
    let again =
        agency_core::web_push::vapid_key(&opened_with(dir.path(), Some(WebPushConfig::default())))
            .unwrap();
    assert_eq!(first, again, "the key is kept in the data directory");
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(dir.path().join("node/web-push-vapid.key"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);

    let hosts = |list: &[&str]| WebPushConfig {
        subject: None,
        extra_hosts: list.iter().map(|one| (*one).to_owned()).collect(),
    };
    let valid = opened_with(dir.path(), Some(hosts(&["push.acme.example"])));
    assert!(agency_core::web_push::vapid_key(&valid).is_some());
    let invalid = opened_with(dir.path(), Some(hosts(&["push.acme.example", "0x7f.1"])));
    assert!(
        agency_core::web_push::vapid_key(&invalid).is_none(),
        "the node does not send with a list that has an address in it"
    );
}

#[test]
fn the_node_warns_when_push_services_would_be_given_a_contact_they_refuse() {
    use agency_core::config::WebPushConfig;
    let dir = tempfile::tempdir().unwrap();
    let warning = |web_push: WebPushConfig, url: &str| {
        let node = opened_with(dir.path(), Some(web_push));
        node.update_config(|config| config.url = url.to_owned())
            .unwrap();
        agency_node::web_push_subject_warning(&node.config())
    };
    let default = || WebPushConfig::default();
    let text =
        warning(default(), "http://127.0.0.1:4400").expect("an http origin is not a contact");
    assert!(
        text.contains("webPush.subject") && text.contains("mailto:"),
        "{text}"
    );
    assert!(
        warning(default(), "https://node.acme.example").is_none(),
        "an https origin is one"
    );
    let mailto = WebPushConfig {
        subject: Some("mailto:ops@acme.example".into()),
        extra_hosts: vec![],
    };
    assert!(
        warning(mailto, "http://127.0.0.1:4400").is_none(),
        "a subject that is set is the operator's"
    );
}

#[tokio::test]
async fn me_says_when_the_node_hides_titles_and_otherwise_says_nothing() {
    let (alice, _) = alice().await;
    let me = |alice: &Running| {
        let request = get(alice, "/api/me").send();
        async move { body(request.await.unwrap()).await.1 }
    };
    assert!(
        me(&alice).await.get("hidesTitles").is_none(),
        "left out, never false"
    );
    alice
        .node
        .update_config(|config| config.hide_titles = true)
        .unwrap();
    assert_eq!(me(&alice).await["hidesTitles"], true);
    alice
        .node
        .update_config(|config| config.hide_titles = false)
        .unwrap();
    assert!(me(&alice).await.get("hidesTitles").is_none());
    let push = agency_core::config::PushConfig {
        key_path: "/nonexistent.p8".into(),
        key_id: "ABCDE12345".into(),
        team_id: "ABCDE12345".into(),
        bundle_id: "com.example.agency".into(),
        hide_titles: true,
    };
    alice
        .node
        .update_config(|config| config.push = Some(push))
        .unwrap();
    assert_eq!(
        me(&alice).await["hidesTitles"],
        true,
        "push.hideTitles means the same"
    );
}
