//! A team's `delivery.host` is checked when the node starts, naming the setting, since it goes to `gh` as
//! an argument.
use agency_adapters::files::DataDir;
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::ports::ConfigStore;
use agency_node::Overrides;
use agency_proto::Id;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn team(name: &str) -> HostedTeam {
    HostedTeam::new(
        id("acme"),
        id(name),
        id("alice"),
        RuntimeCommand::of(vec!["scrum".into(), "runtime".into()]),
    )
}

/// Opens a node whose teams are `teams`, and says why it did not start, if it did not.
fn open_with(teams: Vec<HostedTeam>) -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    let mut config = NodeConfig::new("http://127.0.0.1:1", "127.0.0.1:1", account, "scrum");
    config.teams = teams;
    data.save_config(&config).unwrap();
    agency_node::open(data, Overrides::default())
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tokio::test]
async fn a_host_that_is_not_a_host_name_stops_the_node_and_names_the_setting() {
    for bad in [
        "https://git.example.test",
        "--hostname",
        "git.example.test:8443",
        "",
    ] {
        let teams = vec![
            team("web").with_delivery(vec!["acme/web".into()]),
            team("app").with_delivery_on(bad, vec!["acme/app".into()]),
        ];
        let error = open_with(teams).unwrap_err();
        assert!(
            error.starts_with("teams[1].delivery.host is not a host name"),
            "{bad:?}: {error}"
        );
    }
}

#[tokio::test]
async fn a_team_with_a_host_name_or_none_starts() {
    let teams = vec![
        team("web"),
        team("app").with_delivery(vec!["acme/app".into()]),
        team("api").with_delivery_on("git.example.test", vec!["acme/api".into()]),
        team("ops").with_delivery_on("github.com", vec!["acme/ops".into()]),
    ];
    assert_eq!(open_with(teams), Ok(()));
}
