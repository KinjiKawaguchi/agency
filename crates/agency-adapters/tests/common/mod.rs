//! What the runtime tests share: a fake agency-runtime/1 runtime (a shell script that records each request and
//! answers with the response file prepared for that op), the samples it answers with, and a decision to send it.
//! Each test file uses a part of it.
#![allow(dead_code)]
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_proto::{EnvelopeKind, Id, Payload, RuntimeDecision};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

pub const SAMPLE: &str = include_str!("../../../../testdata/teamview.sample.json");
pub const USAGE: &str = include_str!("../../../../testdata/tokenusage.sample.json");
pub const PARTICIPANTS: &str = include_str!("../../../../testdata/participantmetrics.sample.json");
pub const ACTIVITY: &str = include_str!("../../../../testdata/activity.sample.json");
pub const TICKETS: &str = include_str!("../../../../testdata/tickets.sample.json");

/// The fake runtime's script. It is run as `sh runtime.sh`, never executed itself: a program that was just
/// written cannot be executed while any process still holds it open for writing (`ETXTBSY`, "Text file
/// busy"), and another test's `fork` can hold that descriptor for a moment, which failed a test now and then.
/// A file that only `sh` reads has no such wait, and the shell it starts is never written by a test.
pub const FAKE: &str = r#"input=$(cat)
op=$(printf '%s' "$input" | sed -n 's/.*"op":"\([a-zA-Z]*\)".*/\1/p')
printf '%s' "$input" > "$FAKE_DIR/last-$op.json"
echo "$op" >> "$FAKE_DIR/calls"
answer=$(cat "$FAKE_DIR/$op.json")
sleep "$(cat "$FAKE_DIR/$op.delay" 2>/dev/null || echo 0)"
printf '%s' "$answer"
exit "$(cat "$FAKE_DIR/$op.exit" 2>/dev/null || echo 0)"
"#;

pub fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

pub struct Fake {
    dir: TempDir,
}

impl Fake {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("runtime.sh");
        fs::write(&script, FAKE).unwrap();
        let fake = Self { dir };
        fake.answer("describe", r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"App"}}"#, 0);
        fake.answer("view", &format!(r#"{{"result":{SAMPLE}}}"#), 0);
        fake.answer("decide", r#"{"result":{}}"#, 0);
        fake
    }

    /// Makes the runtime take `seconds` (a number, maybe fractional) to answer `op`.
    pub fn delay(&self, op: &str, seconds: &str) {
        fs::write(self.dir.path().join(format!("{op}.delay")), seconds).unwrap();
    }

    pub fn answer(&self, op: &str, response: &str, exit: u8) {
        fs::write(self.dir.path().join(format!("{op}.json")), response).unwrap();
        fs::write(self.dir.path().join(format!("{op}.exit")), exit.to_string()).unwrap();
    }

    pub fn team(&self, team: &str) -> HostedTeam {
        let env = BTreeMap::from([("FAKE_DIR".to_owned(), self.dir.path().display().to_string())]);
        let script = self.dir.path().join("runtime.sh");
        let runtime = RuntimeCommand::of(vec!["/bin/sh".to_owned(), script.display().to_string()])
            .with_env(env);
        HostedTeam::new(id("acme"), id(team), id("alice"), runtime)
    }

    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join(name)).unwrap_or_default()
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

pub fn decision(text: &str) -> RuntimeDecision {
    RuntimeDecision {
        kind: EnvelopeKind::Answer,
        target: "r-20261124-110000-a1b2".into(),
        by: id("bob"),
        by_participant: None,
        on_behalf_of: None,
        payload: Payload {
            text: text.into(),
            choice: Some("a".into()),
        },
    }
}

pub fn activity_fake(truncated: bool) -> Fake {
    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","activity"],"label":"App"}}"#,
        0,
    );
    let answer = ACTIVITY.replace(
        "\"truncated\": false",
        &format!("\"truncated\": {truncated}"),
    );
    fake.answer("activity", &format!(r#"{{"result":{answer}}}"#), 0);
    fake
}

pub fn at(raw: &str) -> agency_proto::Timestamp {
    agency_proto::Timestamp::parse(raw).unwrap()
}
