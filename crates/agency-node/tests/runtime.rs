//! The command runtime against a fake agency-runtime/1 runtime: a shell script that records each request
//! and answers with the response file prepared for that op.
use agency_node::config::{HostedTeam, RuntimeCommand};
use agency_node::runtime::Runtimes;
use agency_proto::{EnvelopeKind, Id, Payload, RuntimeDecision};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

const SAMPLE: &str = include_str!("../../../testdata/teamview.sample.json");

const FAKE: &str = r#"#!/bin/sh
input=$(cat)
op=$(printf '%s' "$input" | sed -n 's/.*"op":"\([a-z]*\)".*/\1/p')
printf '%s' "$input" > "$FAKE_DIR/last-$op.json"
echo "$op" >> "$FAKE_DIR/calls"
cat "$FAKE_DIR/$op.json"
exit "$(cat "$FAKE_DIR/$op.exit" 2>/dev/null || echo 0)"
"#;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

struct Fake {
    dir: TempDir,
}

impl Fake {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("runtime.sh");
        fs::write(&script, FAKE).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let fake = Self { dir };
        fake.answer("describe", r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"App"}}"#, 0);
        fake.answer("view", &format!(r#"{{"result":{SAMPLE}}}"#), 0);
        fake.answer("decide", r#"{"result":{}}"#, 0);
        fake
    }

    fn answer(&self, op: &str, response: &str, exit: u8) {
        fs::write(self.dir.path().join(format!("{op}.json")), response).unwrap();
        fs::write(self.dir.path().join(format!("{op}.exit")), exit.to_string()).unwrap();
    }

    fn team(&self, team: &str) -> HostedTeam {
        let env = BTreeMap::from([("FAKE_DIR".to_owned(), self.dir.path().display().to_string())]);
        let runtime = RuntimeCommand { command: vec![self.dir.path().join("runtime.sh").display().to_string()], env };
        HostedTeam { org: id("acme"), team: id(team), owner: id("alice"), people: BTreeMap::new(), runtime }
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join(name)).unwrap_or_default()
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

fn decision(text: &str) -> RuntimeDecision {
    RuntimeDecision { kind: EnvelopeKind::Answer, target: "r-20261124-110000-a1b2".into(), by: id("bob"), payload: Payload { text: text.into(), choice: Some("a".into()) } }
}

#[tokio::test]
async fn views_and_decides_over_stdin_and_stdout() {
    let fake = Fake::new();
    let runtimes = Runtimes::command();
    let team = fake.team("app");

    let view = runtimes.view(&team).await.unwrap();
    assert_eq!(view.requests.len(), 3);
    runtimes.decide(&team, decision("--not-a-flag")).await.unwrap();

    assert_eq!(fake.read("calls"), "describe\nview\ndecide\n", "describe once per team, before anything else");
    let request: serde_json::Value = serde_json::from_str(&fake.read("last-decide.json")).unwrap();
    assert_eq!(request["runtime"], "agency-runtime/1");
    assert_eq!((request["org"].as_str(), request["team"].as_str()), (Some("acme"), Some("app")));
    assert_eq!(request["decision"]["payload"]["text"], "--not-a-flag", "decision text travels on stdin");
}

#[tokio::test]
async fn refuses_a_runtime_that_does_not_speak_agency_runtime_1() {
    let fake = Fake::new();
    fake.answer("describe", r#"{"result":{"runtimes":["agency-runtime/2"],"ops":["view"],"label":"App"}}"#, 0);
    let error = Runtimes::command().view(&fake.team("app")).await.unwrap_err();
    assert_eq!(error.status, 503);
    assert!(!fake.read("calls").contains("view"), "no view call to a runtime of another version");
}

#[tokio::test]
async fn refuses_a_view_of_another_team() {
    let fake = Fake::new();
    let error = Runtimes::command().view(&fake.team("other")).await.unwrap_err();
    assert_eq!(error.status, 503);
    assert!(error.message.contains("acme/app"), "{}", error.message);
}

#[tokio::test]
async fn passes_runtime_errors_through_and_treats_garbage_as_unavailable() {
    let fake = Fake::new();
    let runtimes = Runtimes::command();
    let team = fake.team("app");
    fake.answer("decide", r#"{"error":{"status":404,"message":"no such request"}}"#, 1);
    let error = runtimes.decide(&team, decision("")).await.unwrap_err();
    assert_eq!((error.status, error.message.as_str()), (404, "no such request"));

    fake.answer("decide", "Traceback: something broke", 1);
    assert_eq!(runtimes.decide(&team, decision("")).await.unwrap_err().status, 503);

    let missing = HostedTeam { runtime: RuntimeCommand { command: vec![fake.path().join("missing").display().to_string()], env: BTreeMap::new() }, ..team };
    assert_eq!(Runtimes::command().view(&missing).await.unwrap_err().status, 503);
}
