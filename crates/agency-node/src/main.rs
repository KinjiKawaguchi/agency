//! `agency`: the CLI of Agency's node. Creates a node and orgs, invites, joins, adds devices, decides, and serves.
mod dept_cli;
mod device_cli;
mod directory_cli;
mod funding_cli;
mod kpi_cli;
mod logs_cli;
mod outcome_cli;
mod peer_cli;
mod recovery_cli;
mod root_sign_cli;
mod seat_cli;
mod seen_cli;
mod team_cli;

use agency_adapters::files::DataDir;
use agency_core::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::decisions::Decided;
use agency_core::invites::{finish_joins, join, Joined};
use agency_core::logging_config::LogLevel;
use agency_core::outbox::{deliver_due, pending};
use agency_core::peer_about;
use agency_core::ports::ConfigStore;
use agency_core::push::notify_waiting;
use agency_core::ships::record_all;
use agency_core::signed_decisions::decide_with_device;
use agency_core::views::refresh;
use agency_core::{invites, orgs, Node};
use agency_node::Overrides;
use agency_proto::org::{is_node_origin, TeamPlace};
use agency_proto::{EnvelopeKind, Id, Payload, Role};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use device_cli::{device_add, CliDevice};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "agency",
    version,
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
        /// The department the team is in
        #[arg(long)]
        department: Option<String>,
    },
    DeptAdd(dept_cli::DeptAdd),
    DeptSet(dept_cli::DeptSet),
    DeptRemove(dept_cli::DeptRemove),
    Directory(directory_cli::DirectoryArgs),
    SeatAdd(seat_cli::SeatAdd),
    SeatSet(seat_cli::SeatSet),
    SeatRemove(seat_cli::SeatRemove),
    ExternalAdd(seat_cli::ExternalAdd),
    ExternalSet(seat_cli::ExternalSet),
    ExternalRemove(seat_cli::ExternalRemove),
    TeamDept(dept_cli::TeamDept),
    TeamPlan(dept_cli::TeamPlanArgs),
    OutcomeAdd(outcome_cli::OutcomeAdd),
    OutcomeList(outcome_cli::OutcomeList),
    KpiAdd(kpi_cli::KpiAdd),
    KpiSet(kpi_cli::KpiSet),
    KpiRemove(kpi_cli::KpiRemove),
    PlanAim(kpi_cli::PlanAim),
    PlanUnaim(kpi_cli::PlanUnaim),
    TeamSet(team_cli::TeamSet),
    TeamRemove(team_cli::TeamRemove),
    RootSign(root_sign_cli::RootSign),
    RootRecovery(recovery_cli::RootRecovery),
    RootRecover(recovery_cli::RootRecover),
    RootCancelRecovery(recovery_cli::RootCancelRecovery),
    /// Moves an org to a new root key (on the root key's node): the next version names it, signed by the
    /// current root key; the old key is forgotten once it is published
    RootRotate {
        #[arg(long)]
        org: String,
    },
    /// Moves a team's place in the org to another node origin (on the root key's node), for example after
    /// this node's URL changed
    TeamMove {
        #[arg(long)]
        org: String,
        #[arg(long)]
        team: String,
        /// The node origin other nodes reach the team at (http(s)://host[:port])
        #[arg(long)]
        node: String,
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
    /// Adds a device that asked through the web client, after you compare its pairing code. With --cli,
    /// adds this terminal's own device, which `decide` signs with. The account key adds only your first
    /// device; later ones are added from one of your devices.
    DeviceAdd {
        /// Adds the CLI device (its key stays in the data directory)
        #[arg(long)]
        cli: bool,
    },
    /// Answers a request, signed with the CLI device (applied directly to a team on this node; delivered to a team on another node)
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
    /// Tells the program that launches agent sessions which funding source to run on (reads no key)
    Funding {
        #[command(subcommand)]
        command: funding_cli::FundingCommand,
    },
    /// Prints the node's log (logs/node.jsonl and the closed files), oldest first, filtered; reads no key and needs no running node
    Logs(logs_cli::LogsArgs),
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

/// `path` made absolute against the current directory, so it means the same when the node runs from
/// another directory (a launchd service runs from `/`).
fn absolute(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path).with_context(|| format!("cannot make {} absolute", path.display()))
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
    let owner = Account {
        id: id(account)?,
        label: label.to_owned(),
    };
    data.save_config(&NodeConfig::new(url, listen, owner, scrum))?;
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
            agency_proto::org::printable(&doc.body.label),
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
        let held = agency_core::pending_root::waiting(node)
            .unwrap_or_default()
            .into_iter()
            .filter(|one| one.org == doc.body.org)
            .count();
        if held > 0 {
            println!(
                "  {held} change(s) wait for the root key: run `agency root-sign --org {}`.",
                doc.body.org
            );
        }
        if let Some(pending) = &doc.body.root.pending {
            let at = orgs::recovery_takes_effect(node, &doc)
                .map(agency_proto::clock::format)
                .unwrap_or_else(|| "a time this node has no record of".to_owned());
            let what = if pending.public_key.is_some() {
                "a new root key"
            } else {
                "a new recovery key"
            };
            println!(
                "  A RECOVERY WAITS: it moves {} to {what} at {at}. If you did not start it, run `agency root-cancel-recovery --org {}` now.",
                doc.body.org, doc.body.org
            );
        }
        let lines = dept_cli::department_lines(&doc.body);
        let plans = dept_cli::plan_lines(&doc.body);
        for line in lines
            .into_iter()
            .chain(plans)
            .chain(kpi_cli::kpi_lines(&doc.body))
            .chain(seat_cli::seat_lines(&doc.body))
            .chain(peer_cli::peer_lines(node, &doc.body.org))
        {
            println!("{line}");
        }
    }
    for line in seen_cli::seen_lines(node) {
        println!("{line}");
    }
    for doc in node.store().orgs() {
        let devices = doc
            .body
            .member(&config.account.id)
            .map(|person| person.devices.clone())
            .unwrap_or_default();
        for device in devices {
            println!(
                "Device in {}: {} ({}), pairing code {}",
                doc.body.org,
                agency_proto::org::printable(&device.label),
                device.id,
                device.public_key.pairing_code()
            );
        }
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
    warn_misplaced(node, |line| println!("{line}"));
    warn_unknown_addressees(node, |line| println!("{line}"));
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

/// Says which teams run here while their org places them elsewhere, and how to fix it.
fn warn_misplaced(node: &Node, write: impl Fn(String)) {
    for misplaced in orgs::misplaced_teams(node) {
        let Some(place) = misplaced.place else {
            write(format!(
                "team {}/{} is in this node's config.json, but org {} does not list it, so it is not served; if the org removed it, remove it from config.json",
                misplaced.org, misplaced.team, misplaced.org
            ));
            continue;
        };
        write(format!(
            "team {}/{} runs on this node ({}), but the org places it at {place}; other nodes look for it there. On the root key's node, run: agency team-move --org {} --team {} --node {}",
            misplaced.org, misplaced.team, node.url(), misplaced.org, misplaced.team, node.url()
        ));
    }
}

/// Says which entries of a team's `people` name no member, seat, or external party of its org, so that a
/// request addressed to one is not left for no one. It is a warning: a typo here must not stop a node.
fn warn_unknown_addressees(node: &Node, write: impl Fn(String)) {
    for unknown in orgs::unknown_addressees(node) {
        write(format!(
            "team {}/{}: the people entry \"{}\" names {}, who is not a member, seat, or external party of org {}, so a request addressed to {} is answered by no one",
            unknown.org, unknown.team, unknown.name, unknown.id, unknown.org, unknown.name
        ));
    }
}

fn confirm_join(definition: &agency_proto::OrgDefinition) -> bool {
    print!(
        "Join the org \"{}\" ({})? The fingerprint of root key {} matches the link. [y/N] ",
        agency_proto::org::printable(&definition.label),
        definition.org,
        definition.root.account
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
    let logger = opened.logger().clone();
    let recorder = opened.recorder().clone();
    let flushing = recorder.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let flushing = flushing.clone();
            let _ = tokio::task::spawn_blocking(move || flushing.flush()).await;
        }
    });
    logger.record(
        LogLevel::Info,
        "start",
        &format!("node running: {} (listening on {listen})", node.url()),
    );
    warn_misplaced(&node, |line| logger.warn("placement", &line));
    warn_unknown_addressees(&node, |line| logger.warn("placement", &line));
    for line in agency_core::client::home_nodes(&node.config()).1 {
        logger.warn("placement", &line);
    }
    println!("Web client: http://127.0.0.1:{port}/");
    opened.start_usage_collector();
    let background = node.clone();
    let background_log = logger.clone();
    tokio::spawn(async move {
        let logger = background_log;
        let mut tick = 0u64;
        loop {
            if tick.is_multiple_of(2) {
                // Expired held changes are dropped, and a held join's invite freed, even when nobody looks.
                if let Err(error) = agency_core::pending_root::waiting(&background) {
                    logger.warn("root", &format!("sweeping held changes failed: {error}"));
                }
                if let Err(error) = finish_joins(&background).await {
                    logger.warn("joins", &format!("asking for held joins failed: {error}"));
                }
                if let Err(error) = refresh(&background).await {
                    logger.warn(
                        "replicas",
                        &format!("re-reading the replicas failed: {error}"),
                    );
                }
                if let Err(error) = record_all(&background).await {
                    eprintln!("agency: remembering the days epics shipped failed: {error}");
                }
                match notify_waiting(&background).await {
                    Ok(pass) => {
                        for recovery in pass.recoveries {
                            logger.warn("recovery", &recovery);
                            eprintln!("agency: {recovery}");
                        }
                        for failure in pass.failures {
                            logger.warn(
                                "push",
                                &format!("a push notification was not sent: {failure}"),
                            );
                        }
                    }
                    Err(error) => logger.warn(
                        "requests",
                        &format!("checking for new requests failed: {error}"),
                    ),
                }
            }
            if let Err(error) = deliver_due(&background).await {
                logger.warn("outbox", &format!("retrying the outbox failed: {error}"));
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
    let _ = tokio::task::spawn_blocking(move || recorder.flush()).await;
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
        return init(&data, account, label, url, listen, harness_command(scrum)?);
    }
    if let Command::Funding { command } = &cli.command {
        let none = funding_cli::run(&data, command)?;
        std::process::exit(if none { 3 } else { 0 });
    }
    if let Command::Logs(args) = &cli.command {
        return logs_cli::run(&data, args);
    }
    let cli_data = data.clone();
    let opened = agency_node::open(data, Overrides::default())?;
    let node = opened.node.clone();
    match cli.command {
        Command::Init { .. } | Command::Funding { .. } | Command::Logs(_) => unreachable!(),
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
            department,
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
            if !is_here {
                peer_about::read_origin(&node, &org, &place_url).await?;
            }
            let doc = orgs::add_team(
                &node,
                &org,
                TeamPlace::new(team.clone(), label, place_url)
                    .with_department(department.as_deref().map(id).transpose()?),
            )?;
            if let (true, Some(state_dir)) = (is_here, state_dir) {
                let owner = owner
                    .as_deref()
                    .map(id)
                    .transpose()?
                    .unwrap_or_else(|| node.account());
                let runtime = RuntimeCommand::scrum(&node.config().scrum, &absolute(&state_dir)?);
                node.update_config(|config| {
                    config
                        .teams
                        .push(HostedTeam::new(org.clone(), team.clone(), owner, runtime))
                })?;
            }
            println!(
                "Added team {team} to org {org} (version {})",
                doc.body.version
            );
        }
        Command::DeptAdd(args) => dept_cli::dept_add(&node, args)?,
        Command::DeptSet(args) => dept_cli::dept_set(&node, args)?,
        Command::DeptRemove(args) => dept_cli::dept_remove(&node, args)?,
        Command::Directory(args) => directory_cli::run(&node, args)?,
        Command::SeatAdd(args) => seat_cli::seat_add(&node, args)?,
        Command::SeatSet(args) => seat_cli::seat_set(&node, args)?,
        Command::SeatRemove(args) => seat_cli::seat_remove(&node, args)?,
        Command::ExternalAdd(args) => seat_cli::external_add(&node, args)?,
        Command::ExternalSet(args) => seat_cli::external_set(&node, args)?,
        Command::ExternalRemove(args) => seat_cli::external_remove(&node, args)?,
        Command::TeamDept(args) => dept_cli::team_dept(&node, args)?,
        Command::TeamPlan(args) => dept_cli::team_plan(&node, args)?,
        Command::OutcomeAdd(args) => outcome_cli::outcome_add(&node, args).await?,
        Command::OutcomeList(args) => outcome_cli::outcome_list(&node, args)?,
        Command::KpiAdd(args) => kpi_cli::kpi_add(&node, args)?,
        Command::KpiSet(args) => kpi_cli::kpi_set(&node, args)?,
        Command::KpiRemove(args) => kpi_cli::kpi_remove(&node, args)?,
        Command::PlanAim(args) => kpi_cli::plan_aim(&node, args)?,
        Command::PlanUnaim(args) => kpi_cli::plan_unaim(&node, args)?,
        Command::TeamSet(args) => team_cli::team_set(&node, args)?,
        Command::TeamRemove(args) => team_cli::team_remove(&node, args)?,
        Command::RootSign(args) => root_sign_cli::root_sign(&node, args).await?,
        Command::RootRecovery(args) => recovery_cli::root_recovery(&node, args).await?,
        Command::RootRecover(args) => recovery_cli::root_recover(&node, args).await?,
        Command::RootCancelRecovery(args) => recovery_cli::root_cancel_recovery(&node, args)?,
        Command::RootRotate { org } => {
            let doc = orgs::rotate_root(&node, &id(&org)?)?;
            println!(
                "Org {org} has a new root key, fingerprint {} (version {})",
                doc.body.root.public_key.fingerprint(),
                doc.body.version
            );
        }
        Command::TeamMove {
            org,
            team,
            node: origin,
        } => {
            if origin != node.url() {
                peer_about::read_origin(&node, &id(&org)?, &origin).await?;
            }
            let doc = orgs::move_team(&node, &id(&org)?, &id(&team)?, origin.clone())?;
            println!(
                "Moved team {team} of org {org} to {origin} (version {})",
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
            match join(&node, &link, |definition| yes || confirm_join(definition)).await? {
                Joined::Member(doc) => println!(
                    "Joined org {} ({}) (version {})",
                    doc.body.org, agency_proto::org::printable(&doc.body.label), doc.body.version
                ),
                Joined::Waiting(message) => println!(
                    "The org's root key's node holds the join for its owner. {message} This node asks again on its own, and with `agency sync`."
                ),
            }
        }
        Command::Decide {
            org,
            team,
            request,
            choice,
            text,
        } => {
            let device = CliDevice::load(&cli_data)?;
            let outcome = decide_with_device(
                &node,
                device.signer(),
                &id(&org)?,
                &id(&team)?,
                EnvelopeKind::Answer,
                request,
                Payload { text, choice },
            )
            .await
            .context("is the CLI device added in this org? `agency status` lists your devices")?;
            println!(
                "{}",
                match outcome {
                    Decided::Applied => "Applied to the team on this node".to_owned(),
                    Decided::Delivered => "Delivered to the team's node".to_owned(),
                    Decided::Queued(why) => format!(
                        "The team's node is unreachable; the decision waits in the outbox ({why})"
                    ),
                    Decided::Refused(why) => format!("The team's node refused: {why}"),
                }
            );
        }
        Command::DeviceAdd { cli } => device_add(&node, &cli_data, cli).await?,
        Command::Sync => {
            refresh(&node).await?;
            for joined in finish_joins(&node).await? {
                if let Joined::Member(doc) = joined {
                    println!("Joined org {} (version {})", doc.body.org, doc.body.version);
                }
            }
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
