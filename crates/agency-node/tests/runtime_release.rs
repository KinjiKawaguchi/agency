//! `agency status` says which harness release each hosted team's runtime reports in `describe`: a runtime that
//! says one is shown with it, one that does not is read as before and said so, and a team not read yet is said
//! so too.
use agency_adapters::files::DataDir;
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::orgs;
use agency_core::ports::ConfigStore;
use agency_core::usage::team_usage;
use agency_node::{open, Overrides};
use agency_proto::org::TeamPlace;
use agency_proto::Id;
use std::process::Command;
use tempfile::TempDir;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

/// A team whose runtime answers every call with `answer`, a response of agency-runtime/1.
fn team_answering(name: &str, answer: &str) -> HostedTeam {
    let script = format!("cat > /dev/null; printf '%s' '{answer}'");
    let runtime = RuntimeCommand::of(vec!["/bin/sh".into(), "-c".into(), script]);
    HostedTeam::new(id("acme"), id(name), id("alice"), runtime)
}

fn describe(extra: &str) -> String {
    format!(
        r#"{{"result":{{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"App"{extra}}}}}"#
    )
}

fn status_of(data: &DataDir) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_agency"))
        .arg("--dir")
        .arg(data.root())
        .arg("status")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn team_line(status: &str, team: &str) -> String {
    let prefix = format!("Team on this node: acme/{team} ");
    status
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no line for {team}: {status}"))
        .to_owned()
}

#[tokio::test]
async fn status_says_the_release_each_runtime_reported_and_which_did_not() {
    let root = TempDir::new().unwrap();
    let data = DataDir::new(root.path().join("node"));
    data.create_key().unwrap();
    let account = Account {
        id: id("alice"),
        label: "Alice".into(),
    };
    let mut config = NodeConfig::new("http://127.0.0.1:1", "127.0.0.1:1", account, "scrum");
    let teams = [
        team_answering("said", &describe(r#","version":"0.7.2""#)),
        team_answering("silent", &describe("")),
        team_answering(
            "toolong",
            &describe(&format!(r#","version":"{}""#, "v".repeat(65))),
        ),
        team_answering("unread", &describe(r#","version":"9.9.9""#)),
    ];
    config.teams = teams.to_vec();
    data.save_config(&config).unwrap();
    let opened = open(data.clone(), Overrides::default()).unwrap();

    orgs::create(&opened.node, id("acme"), "Acme".into()).unwrap();
    for team in &teams {
        let place = TeamPlace::new(team.team.clone(), "team", opened.node.url());
        orgs::add_team(&opened.node, &id("acme"), place).unwrap();
    }
    for team in &teams[..3] {
        let _ = team_usage(&opened.node, &id("acme"), &team.team, Some(7)).await;
    }

    let status = status_of(&data);
    let said = team_line(&status, "said");
    assert!(said.contains("; runtime release 0.7.2, read 20"), "{said}");
    let silent = team_line(&status, "silent");
    assert!(
        silent.contains("; runtime does not say its release, read 20"),
        "{silent}"
    );
    let too_long = team_line(&status, "toolong");
    assert!(
        too_long.contains("; runtime does not say its release, read 20"),
        "a value beyond the limits is ignored: {too_long}"
    );
    let unread = team_line(&status, "unread");
    assert!(
        unread.ends_with("; runtime release not read yet"),
        "{unread}"
    );
}
