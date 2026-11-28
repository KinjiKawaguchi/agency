//! `agency`: the CLI of Agency's node. Creates a node and orgs, invites, joins, adds devices, decides, and serves.
//! Each area's subcommands and handlers are in their own module; this file parses the command line and hands
//! each command to its module.
mod audit_cli;
mod budget_cli;
mod claim_cli;
mod decide_cli;
mod dept_cli;
mod device_cli;
mod directory_cli;
mod funding_cli;
mod init_cli;
mod invite_cli;
mod kpi_cli;
mod logs_cli;
mod member_cli;
mod metrics_cli;
mod org_cli;
mod outcome_cli;
mod peer_cli;
mod recovery_cli;
mod root_sign_cli;
mod seat_cli;
mod seen_cli;
mod serve_cli;
mod status_cli;
mod team_cli;

use agency_adapters::files::DataDir;
use agency_node::Overrides;
use agency_proto::Id;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

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
    Init(init_cli::Init),
    OrgCreate(org_cli::OrgCreate),
    TeamAdd(team_cli::TeamAdd),
    DeptAdd(dept_cli::DeptAdd),
    DeptSet(dept_cli::DeptSet),
    DeptRemove(dept_cli::DeptRemove),
    Directory(directory_cli::DirectoryArgs),
    SeatAdd(seat_cli::SeatAdd),
    SeatSet(seat_cli::SeatSet),
    SeatRemove(seat_cli::SeatRemove),
    Audit(audit_cli::Audit),
    MemberRemove(member_cli::MemberRemove),
    GuestRemove(member_cli::GuestRemove),
    Guests(member_cli::Guests),
    SeatMerge(seat_cli::SeatMerge),
    SeatInvite(claim_cli::SeatInvite),
    /// Lists the claims of seats that a person's browser redeemed and that wait for you to confirm
    Claims,
    ClaimConfirm(claim_cli::ClaimConfirm),
    ClaimReject(claim_cli::ClaimReject),
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
    RootRotate(org_cli::RootRotate),
    TeamMove(team_cli::TeamMove),
    Invite(invite_cli::Invite),
    Join(invite_cli::Join),
    DeviceAdd(device_cli::DeviceAdd),
    Decide(decide_cli::Decide),
    /// Re-reads the replicas once and retries the outbox once
    Sync,
    /// Shows this node: its account, orgs, hosted teams, and outbox
    Status,
    /// Tells the program that launches agent sessions which funding source to run on (reads no key)
    Funding {
        #[command(subcommand)]
        command: funding_cli::FundingCommand,
    },
    Budget(budget_cli::Budget),
    /// Prints the node's log (logs/node.jsonl and the closed files), oldest first, filtered; reads no key and needs no running node
    Logs(logs_cli::LogsArgs),
    /// Prints the node's own measurements (how long routes and calls take) as a table, from the files in the data directory; reads no key and needs no running node
    Metrics(metrics_cli::MetricsArgs),
    Export(agency_node::export::ExportArgs),
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

async fn run(cli: Cli) -> Result<()> {
    let data = data_dir(&cli);
    if let Command::Init(args) = &cli.command {
        return init_cli::run(&data, args);
    }
    data.check_format()?;
    if let Command::Funding { command } = &cli.command {
        let none = funding_cli::run(&data, command)?;
        std::process::exit(if none { 3 } else { 0 });
    }
    if let Command::Logs(args) = &cli.command {
        return logs_cli::run(&data, args);
    }
    if let Command::Metrics(args) = &cli.command {
        return metrics_cli::run(&data, args);
    }
    if let Command::Export(args) = &cli.command {
        if let Some(dir) = args.only_verifies() {
            return agency_node::export::verify_dir(dir);
        }
    }
    let cli_data = data.clone();
    let opened = agency_node::open(data, Overrides::default())?;
    let node = opened.node.clone();
    match cli.command {
        Command::Init(_) | Command::Funding { .. } | Command::Logs(_) | Command::Metrics(_) => {
            unreachable!()
        }
        Command::Budget(args) => budget_cli::run(&node, args).await?,
        Command::OrgCreate(args) => org_cli::org_create(&node, args)?,
        Command::TeamAdd(args) => team_cli::team_add(&node, args).await?,
        Command::DeptAdd(args) => dept_cli::dept_add(&node, args)?,
        Command::DeptSet(args) => dept_cli::dept_set(&node, args)?,
        Command::DeptRemove(args) => dept_cli::dept_remove(&node, args)?,
        Command::Directory(args) => directory_cli::run(&node, args)?,
        Command::SeatAdd(args) => seat_cli::seat_add(&node, args)?,
        Command::SeatSet(args) => seat_cli::seat_set(&node, args)?,
        Command::SeatRemove(args) => seat_cli::seat_remove(&node, args)?,
        Command::Audit(args) => audit_cli::audit(&node, args)?,
        Command::MemberRemove(args) => member_cli::member_remove(&node, args)?,
        Command::GuestRemove(args) => member_cli::guest_remove(&node, args)?,
        Command::Guests(args) => member_cli::guests(&node, args)?,
        Command::SeatMerge(args) => seat_cli::seat_merge(&node, args)?,
        Command::Claims => claim_cli::list(&node),
        Command::ClaimConfirm(args) => claim_cli::confirm(&node, args).await?,
        Command::ClaimReject(args) => claim_cli::reject(&node, args).await?,
        Command::SeatInvite(args) => claim_cli::seat_invite(&node, args)?,
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
        Command::RootRotate(args) => org_cli::root_rotate(&node, args)?,
        Command::TeamMove(args) => team_cli::team_move(&node, args).await?,
        Command::Invite(args) => invite_cli::invite(&node, args)?,
        Command::Join(args) => invite_cli::join_org(&node, args).await?,
        Command::Decide(args) => decide_cli::decide(&node, &cli_data, args).await?,
        Command::DeviceAdd(args) => device_cli::device_add(&node, &cli_data, args).await?,
        Command::Sync => status_cli::sync(&node).await?,
        Command::Status => status_cli::status(&node, &cli_data),
        Command::Export(args) => agency_node::export::run(&node, args)?,
        Command::Serve => serve_cli::serve(opened).await?,
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
