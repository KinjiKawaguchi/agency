//! 2 つの台を同じプロセスで動かし、招待 → 参加 → 写し → 署名した決裁 → ため置き → 起きた後の届け を通す。
//! 断る側 (所属していない人、書き換えた封筒、使った招待、偽の台、根でない鍵の版) も確かめる。
use agency_node::client::{decide, join, refresh, replica_path, Decided, Replica};
use agency_node::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_node::runtime::{MemoryRuntime, Runtimes};
use agency_node::node::Node;
use agency_node::orgs;
use agency_node::outbox::{deliver, deliver_due, pending, ParcelBody};
use agency_node::server::router;
use agency_node::store::{read_json, DataDir};
use agency_proto::invite::Invite;
use agency_proto::org::{SignedOrg, TeamPlace};
use agency_proto::{EnvelopeKind, Id, Kind, OrgDefinition, Payload, Role, Signed};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

struct Running {
    node: Arc<Node>,
    port: u16,
    server: Option<JoinHandle<()>>,
    _dir: TempDir,
}

impl Running {
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    async fn start(&mut self) {
        let listener = TcpListener::bind(("127.0.0.1", self.port)).await.unwrap();
        let app = router(self.node.clone());
        self.server = Some(tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }));
    }

    async fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
            let _ = server.await;
        }
    }
}

async fn node(account: &str, runtimes: Runtimes) -> Running {
    let dir = TempDir::new().unwrap();
    let port = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let url = format!("http://127.0.0.1:{port}");
    data.save_config(&NodeConfig { url, listen: format!("127.0.0.1:{port}"), account: Account { id: id(account), label: account.to_uppercase() }, scrum: "scrum".into(), teams: vec![], trusted: vec![] }).unwrap();
    let mut running = Running { node: Node::open(data, Some(runtimes)).unwrap(), port, server: None, _dir: dir };
    running.start().await;
    running
}

/// alice の台: 組織 acme の根の鍵を持ち、チーム app を置く。チームには答え待ちの要求 r-1 がある
async fn alice() -> (Running, Arc<Mutex<MemoryRuntime>>) {
    let (runtimes, memory) = Runtimes::memory();
    memory.lock().unwrap().requests.push(("r-1".into(), "鍵を作るか".into()));
    let running = node("alice", runtimes).await;
    let node = &running.node;
    orgs::create(node, id("acme"), "Acme".into()).unwrap();
    orgs::add_team(node, &id("acme"), TeamPlace { id: id("app"), label: "app".into(), department: None, node: running.url() }).unwrap();
    node.update_config(|config| config.teams.push(HostedTeam { org: id("acme"), team: id("app"), owner: id("alice"), people: Default::default(), runtime: RuntimeCommand { command: vec!["/nonexistent".into()], env: Default::default() } })).unwrap();
    (running, memory)
}

async fn joined_bob(alice: &Running) -> Running {
    let bob = node("bob", Runtimes::memory().0).await;
    let link = orgs::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let doc = join(&bob.node, &link, |definition| definition.label == "Acme").await.unwrap();
    assert!(doc.body.member(&id("bob")).is_some());
    bob
}

fn answer(choice: &str) -> Payload {
    Payload { text: String::new(), choice: Some(choice.into()) }
}

#[tokio::test]
async fn invite_join_replica_and_signed_decision() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    // alice の台も、bob を書き足した版を持っている (書き手は alice の根の鍵 1 つ)
    assert!(alice.node.data.load_org(&id("acme")).unwrap().body.member(&id("bob")).is_some());

    refresh(&bob.node).await.unwrap();
    let replica: Replica = read_json(&replica_path(&bob.node, &id("acme"), &id("app"))).unwrap();
    assert_eq!(replica.error, None);
    assert_eq!(replica.view.unwrap().requests[0].id, "r-1");

    let outcome = decide(&bob.node, &id("acme"), &id("app"), EnvelopeKind::Answer, "r-1".into(), answer("ア")).await.unwrap();
    assert_eq!(outcome, Decided::Delivered);
    let applied = memory.lock().unwrap().applied.clone();
    assert_eq!(applied.len(), 1);
    assert_eq!((applied[0].by.as_str(), applied[0].payload.choice.as_deref()), ("bob", Some("ア")), "答えた人は封筒のアカウントの人");

    // A request the team runtime does not know is a refusal: not retried, not queued
    let outcome = decide(&bob.node, &id("acme"), &id("app"), EnvelopeKind::Answer, "r-404".into(), answer("ア")).await.unwrap();
    assert!(matches!(outcome, Decided::Refused(_)), "{outcome:?}");
    assert!(pending(&bob.node).is_empty());
}

#[tokio::test]
async fn decisions_wait_while_the_team_node_sleeps_and_arrive_after() {
    let (mut alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.stop().await;
    let outcome = decide(&bob.node, &id("acme"), &id("app"), EnvelopeKind::Answer, "r-1".into(), answer("イ")).await.unwrap();
    assert!(matches!(outcome, Decided::Queued(_)), "{outcome:?}");
    assert_eq!(pending(&bob.node).len(), 1);
    refresh(&bob.node).await.unwrap();
    let replica: Replica = read_json(&replica_path(&bob.node, &id("acme"), &id("app"))).unwrap_or_default();
    assert!(replica.error.is_some(), "寝ている台の写しは失敗を書く");

    alice.start().await;
    assert!(deliver_due(&bob.node).await.unwrap().is_empty(), "送り直しの時刻の前には届けに行かない");
    let parcel = pending(&bob.node).remove(0);
    deliver(&bob.node, &parcel).await.unwrap();
    assert!(pending(&bob.node).is_empty());
    assert_eq!(memory.lock().unwrap().applied.len(), 1);

    // 同じ封筒の 2 度目は当てない (送り直しで 2 回届いた写し)
    let ParcelBody::Envelope(signed) = parcel.body else { panic!("封筒のはず") };
    let signed = *signed;
    let again = reqwest::Client::new().post(format!("{}/fed/v1/teams/acme/app/envelopes", alice.url())).json(&signed).send().await.unwrap();
    assert_eq!(again.json::<serde_json::Value>().await.unwrap()["duplicate"], true);
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
}

#[tokio::test]
async fn refuses_strangers_tampering_reused_invites_and_fake_nodes() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    let http = reqwest::Client::new();

    // 所属していない人の署名つきの読み出しは 404 (組織があることも見せない)
    let carol = node("carol", Runtimes::memory().0).await;
    carol.node.data.save_org(&alice.node.data.load_org(&id("acme")).unwrap()).unwrap();
    refresh(&carol.node).await.unwrap();
    assert!(read_json::<Replica>(&replica_path(&carol.node, &id("acme"), &id("app"))).is_none());
    let mut request = http.get(format!("{}/fed/v1/teams/acme/app/view", alice.url()));
    for (name, value) in agency_node::auth::signed_headers(&carol.node.key, &id("carol"), "GET", "/fed/v1/teams/acme/app/view") {
        request = request.header(name, value);
    }
    assert_eq!(request.send().await.unwrap().status(), 404);

    // 書き換えた封筒は 401 で、当てない
    decide(&bob.node, &id("acme"), &id("app"), EnvelopeKind::Answer, "r-1".into(), answer("ア")).await.unwrap();
    let mut tampered: Signed<agency_proto::Envelope> = Signed::sign(Kind::Envelope, agency_proto::Envelope { id: agency_proto::EnvelopeId::random(), kind: EnvelopeKind::Answer, org: id("acme"), team: id("app"), target: "r-1".into(), payload: answer("ア"), account: id("bob"), at: agency_proto::clock::format(agency_proto::clock::now()) }, id("bob"), &bob.node.key).unwrap();
    tampered.body.payload = answer("イ");
    let response = http.post(format!("{}/fed/v1/teams/acme/app/envelopes", alice.url())).json(&tampered).send().await.unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(memory.lock().unwrap().applied.len(), 1);

    // 使った招待では、もう入れない
    let link = orgs::invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let dave = node("dave", Runtimes::memory().0).await;
    assert!(join(&dave.node, &link, |_| true).await.is_err());

    // 指紋の合わない招待のリンク (偽の台) では申し込まない
    let mut fake = Invite::parse(&orgs::invite(&alice.node, &id("acme"), None, Role::Member).unwrap()).unwrap();
    fake.fp = "0".repeat(32);
    let error = join(&dave.node, &fake.to_link(), |_| true).await.unwrap_err();
    assert!(error.to_string().contains("指紋"), "{error}");

    // 根でない鍵で署名した次の版は、受け取らない
    let current = alice.node.data.load_org(&id("acme")).unwrap();
    let mut body = OrgDefinition::successor_of(&current, agency_proto::clock::format(agency_proto::clock::now()));
    body.label = "乗っ取り".into();
    let forged: SignedOrg = Signed::sign(Kind::Org, body, id("alice"), &bob.node.key).unwrap();
    let response = http.post(format!("{}/fed/v1/orgs/acme/definition", alice.url())).json(&forged).send().await.unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(alice.node.data.load_org(&id("acme")).unwrap().body.label, "Acme");
}
