//! `agency`: the CLI of Agency's node. Creates a node and orgs, invites, joins, decides, and serves.
use agency_adapters::files::DataDir;
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::decisions::{decide, Decided};
use agency_core::invites::join;
use agency_core::outbox::{deliver_due, pending};
use agency_core::ports::ConfigStore;
use agency_core::views::refresh;
use agency_core::{invites, orgs, Node};
use agency_node::Overrides;
use agency_proto::org::{is_node_origin, TeamPlace};
use agency_proto::{EnvelopeKind, Id, Payload, Role};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "agency",
    about = "Agency's node: runs organizations of AI agents and people across many machines"
)]
struct Cli {
    /// The data directory (default: $AGENCY_DIR or ~/.agency)
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Creates this node (makes the account key and writes the configuration)
    Init {
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
    },
    /// Creates an org (this node's account becomes its root key)
    OrgCreate {
        #[arg(long)]
        org: String,
        #[arg(long)]
        label: String,
    },
    /// Adds a team to an org (on the root key's node). Without --node, the team runs on this node
    TeamAdd {
        #[arg(long)]
        org: String,
        #[arg(long)]
        team: String,
        #[arg(long)]
        label: String,
        /// For a team on this node: the scrum harness state directory. The team runtime is `<scrum> runtime`.
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// For a team on this node: the org person who owns it (default: this node's account)
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        node: Option<String>,
    },
    /// Creates an invite link to an org (on the root key's node)
    Invite {
        #[arg(long)]
        org: String,
        #[arg(long)]
        department: Option<String>,
        /// member (reads and answers) or answerer (only answers)
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// Joins an org from an invite link
    Join {
        link: String,
        /// Joins without asking for confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Answers a request (applied directly to a team on this node; signed and delivered to a team on another node)
    Decide {
        #[arg(long)]
        org: String,
        #[arg(long)]
        team: String,
        /// The request id
        #[arg(long)]
        request: String,
        #[arg(long)]
        choice: Option<String>,
        /// The answer text (text starting with - is taken as is)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        text: String,
    },
    /// Re-reads the replicas once and retries the outbox once
    Sync,
    /// Shows this node: its account, orgs, hosted teams, and outbox
    Status,
    /// Runs the node: serves the federation routes, the Client API, and the web client, and keeps replicas and the outbox moving
    Serve,
}

fn data_dir(cli: &Cli) -> DataDir {
    let root = cli
        .dir
        .clone()
        .or_else(|| std::env::var_os("AGENCY_DIR").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".agency")
        });
    DataDir::new(root)
}

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

fn init(
    data: &DataDir,
    account: &str,
    label: &str,
    url: &str,
    listen: &str,
    scrum: PathBuf,
) -> Result<()> {
    if data.config_path().exists() {
        bail!("{} already exists", data.config_path().display());
    }
    if !is_node_origin(url) {
        bail!("--url must be http(s)://host[:port]");
    }
    let key = data.create_key()?;
    data.save_config(&NodeConfig {
        url: url.to_owned(),
        listen: listen.to_owned(),
        account: Account {
            id: id(account)?,
            label: label.to_owned(),
        },
        scrum,
        teams: vec![],
        trusted: vec![],
        funding: vec![],
        access: None,
    })?;
    println!(
        "Created the node in {}\nFingerprint of account {account}'s key: {}",
        data.root().display(),
        key.public().fingerprint()
    );
    Ok(())
}

fn status(node: &Node) {
    let config = node.config();
    println!(
        "Account: {} ({}), key fingerprint {}",
        config.account.id,
        config.account.label,
        node.key().public().fingerprint()
    );
    println!("This node: {} (listening on {})", config.url, config.listen);
    for doc in node.store().orgs() {
        let role = if orgs::is_root(node, &doc) {
            "root key"
        } else if doc.body.member(&config.account.id).is_some() {
            "member"
        } else {
            "replica"
        };
        println!(
            "Org {} ({}), version {}, {}, people: {}, teams: {}",
            doc.body.org,
            doc.body.label,
            doc.body.version,
            role,
            doc.body.people.len(),
            doc.body
                .teams
                .iter()
                .map(|t| format!("{}@{}", t.id, t.node))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for team in &config.teams {
        println!(
            "Team on this node: {}/{} (owner {}) runs {}",
            team.org,
            team.team,
            team.owner,
            team.runtime.command.join(" ")
        );
    }
    for parcel in pending(node) {
        println!(
            "Outbox: {} -> {} ({} attempts, {})",
            parcel.id,
            parcel.to,
            parcel.attempts,
            parcel.last_error.unwrap_or_default()
        );
    }
}

fn confirm_join(definition: &agency_proto::OrgDefinition) -> bool {
    print!(
        "Join the org \"{}\" ({})? The fingerprint of root key {} matches the link. [y/N] ",
        definition.label, definition.org, definition.root.account
    );
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
}

async fn serve(opened: agency_node::Opened) -> Result<()> {
    let node = opened.node.clone();
    let listen = node.config().listen;
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .with_context(|| format!("cannot listen on {listen}"))?;
    let port = listener.local_addr()?.port();
    let app = opened.app(port)?;
    println!("Node running: {} (listening on {listen})", node.url());
    println!("Web client: http://127.0.0.1:{port}/");
    let background = node.clone();
    tokio::spawn(async move {
        let mut tick = 0u64;
        loop {
            if tick.is_multiple_of(2) {
                if let Err(error) = refresh(&background).await {
                    eprintln!("agency: re-reading the replicas failed: {error}");
                }
            }
            if let Err(error) = deliver_due(&background).await {
                eprintln!("agency: retrying the outbox failed: {error}");
            }
            tick += 1;
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}

async fn run(cli: Cli) -> Result<()> {
    let data = data_dir(&cli);
    if let Command::Init {
        account,
        label,
        url,
        listen,
        scrum,
    } = &cli.command
    {
        return init(&data, account, label, url, listen, scrum.clone());
    }
    let opened = agency_node::open(data, Overrides::default())?;
    let node = opened.node.clone();
    match cli.command {
        Command::Init { .. } => unreachable!(),
        Command::OrgCreate { org, label } => {
            let doc = orgs::create(&node, id(&org)?, label)?;
            println!(
                "Created org {}. Root key fingerprint: {}",
                doc.body.org,
                node.key().public().fingerprint()
            );
        }
        Command::TeamAdd {
            org,
            team,
            label,
            state_dir,
            owner,
            node: place,
        } => {
            let (org, team) = (id(&org)?, id(&team)?);
            let place_url = place.unwrap_or_else(|| node.url());
            if !is_node_origin(&place_url) {
                bail!("--node must be http(s)://host[:port]");
            }
            let is_here = place_url == node.url();
            if is_here && state_dir.is_none() {
                bail!("a team on this node needs --state-dir");
            }
            let doc = orgs::add_team(
                &node,
                &org,
                TeamPlace {
                    id: team.clone(),
                    label,
                    department: None,
                    node: place_url,
                },
            )?;
            if let (true, Some(state_dir)) = (is_here, state_dir) {
                let owner = owner
                    .as_deref()
                    .map(id)
                    .transpose()?
                    .unwrap_or_else(|| node.account());
                let runtime = RuntimeCommand::scrum(&node.config().scrum, &state_dir);
                node.update_config(|config| {
                    config.teams.push(HostedTeam {
                        org: org.clone(),
                        team: team.clone(),
                        owner,
                        people: Default::default(),
                        runtime,
                    })
                })?;
            }
            println!(
                "Added team {team} to org {org} (version {})",
                doc.body.version
            );
        }
        Command::Invite {
            org,
            department,
            role,
        } => {
            let role = match role.as_str() {
                "member" => Role::Member,
                "answerer" => Role::Answerer,
                _ => bail!("--role must be member or answerer"),
            };
            let department = department.as_deref().map(id).transpose()?;
            println!("{}", invites::invite(&node, &id(&org)?, department, role)?);
        }
        Command::Join { link, yes } => {
            let doc = join(&node, &link, |definition| yes || confirm_join(definition)).await?;
            println!(
                "Joined org {} ({}) (version {})",
                doc.body.org, doc.body.label, doc.body.version
            );
        }
        Command::Decide {
            org,
            team,
            request,
            choice,
            text,
        } => {
            let outcome = decide(
                &node,
                &id(&org)?,
                &id(&team)?,
                EnvelopeKind::Answer,
                request,
                Payload { text, choice },
            )
            .await?;
            println!(
                "{}",
                match outcome {
                    Decided::Applied => "Applied to the team on this node".to_owned(),
                    Decided::Delivered => "Signed and delivered to the team's node".to_owned(),
                    Decided::Queued(why) => format!(
                        "The team's node is unreachable; the decision waits in the outbox ({why})"
                    ),
                    Decided::Refused(why) => format!("The team's node refused: {why}"),
                }
            );
        }
        Command::Sync => {
            refresh(&node).await?;
            let outcomes = deliver_due(&node).await?;
            println!(
                "Re-read the replicas and tried {} outbox deliveries",
                outcomes.len()
            );
        }
        Command::Status => status(&node),
        Command::Serve => serve(opened).await?,
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("agency: {error:#}");
        std::process::exit(1);
    }
}
