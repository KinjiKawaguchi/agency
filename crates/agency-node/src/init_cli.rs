//! `agency init`: creates this node, its account key and its configuration. It runs before there is a node to
//! open, so it works on the data directory alone.
use crate::absolute;
use agency_adapters::files::DataDir;
use agency_core::config::{Account, NodeConfig};
use agency_core::ports::ConfigStore;
use agency_proto::org::is_node_origin;
use anyhow::{bail, Result};
use clap::Args;
use std::path::{Path, PathBuf};

/// Creates this node (makes the account key and writes the configuration)
#[derive(Args)]
pub struct Init {
    #[arg(long)]
    account: String,
    #[arg(long)]
    label: String,
    /// Where other nodes reach this node (such as http://127.0.0.1:4400)
    #[arg(long)]
    url: String,
    #[arg(long, default_value = "127.0.0.1:4400")]
    listen: String,
    /// The path of the harness CLI (scrum)
    #[arg(long, default_value = "scrum")]
    scrum: PathBuf,
    /// Refuse every direct local request to the Client API and the web client (`"localClients": false`),
    /// for a machine that other accounts log in to. Federation keeps working
    #[arg(long)]
    no_local_clients: bool,
}

/// What `init` says when the node admits local clients, as every node does unless told otherwise.
const LOCAL_CLIENTS_HINT: &str = "Every process on this machine that reaches 127.0.0.1 is admitted as the node's owner. \
If other accounts log in to this machine, set \"localClients\": false in config.json (or run init with \
--no-local-clients): the Client API and the web client then refuse local requests, and federation keeps working.";

pub fn run(data: &DataDir, args: &Init) -> Result<()> {
    let config = NodeConfig::new(
        &args.url,
        &args.listen,
        owner_of(&args.account, &args.label)?,
        harness_command(&args.scrum)?,
    )
    .with_local_clients(!args.no_local_clients);
    init(data, config)
}

/// The harness CLI to configure: a bare name stays a name looked up on the node's `PATH`; a path is
/// made absolute.
fn harness_command(scrum: &Path) -> Result<PathBuf> {
    let is_bare_name = !scrum.is_absolute() && scrum.components().count() == 1;
    if is_bare_name {
        Ok(scrum.to_owned())
    } else {
        absolute(scrum)
    }
}

fn owner_of(account: &str, label: &str) -> Result<Account> {
    Ok(Account {
        id: crate::id(account)?,
        label: label.to_owned(),
    })
}

fn init(data: &DataDir, config: NodeConfig) -> Result<()> {
    data.check_format()?;
    if data.config_path().exists() {
        bail!("{} already exists", data.config_path().display());
    }
    if !is_node_origin(&config.url) {
        bail!("--url must be http(s)://host[:port]");
    }
    if agency_proto::runtime::is_reserved_name(&config.account.id) {
        bail!(
            "--account {} is a name a team runtime reserves; choose another account id",
            config.account.id
        );
    }
    let key = data.create_key()?;
    let account = config.account.id.clone();
    let admits_local_clients = config.local_clients;
    data.save_config(&config)?;
    data.ensure_format()?;
    println!(
        "Created the node in {}\nFingerprint of account {account}'s key: {}",
        data.root().display(),
        key.public().fingerprint()
    );
    if admits_local_clients {
        println!("{LOCAL_CLIENTS_HINT}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_paths_do_not_depend_on_the_directory_the_node_runs_from() {
        let here = std::env::current_dir().unwrap();
        assert_eq!(absolute(Path::new("state")).unwrap(), here.join("state"));
        assert_eq!(
            harness_command(Path::new("bin/scrum")).unwrap(),
            here.join("bin/scrum")
        );
        assert_eq!(
            harness_command(Path::new("./scrum")).unwrap(),
            here.join("scrum")
        );
        assert_eq!(
            harness_command(Path::new("scrum")).unwrap(),
            Path::new("scrum"),
            "a bare name is looked up on PATH"
        );
        assert_eq!(
            harness_command(Path::new("/opt/scrum")).unwrap(),
            Path::new("/opt/scrum")
        );
    }
}
