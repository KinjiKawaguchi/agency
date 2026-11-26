//! `agency funding`: which funding source an agent session runs on. The harness that launches the
//! sessions calls these and sets the printed `env` for the session. They read the configuration and the
//! usage files only, so they need neither a running node nor the account key, and print no secret.
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_core::config::Tool;
use agency_core::funding_pick::{list, pick, PickState};
use agency_core::ports::ConfigStore;
use agency_proto::clock::OffsetDateTime;
use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
pub enum ToolArg {
    Claude,
    Codex,
}

impl From<ToolArg> for Tool {
    fn from(tool: ToolArg) -> Self {
        match tool {
            ToolArg::Claude => Tool::Claude,
            ToolArg::Codex => Tool::Codex,
        }
    }
}

#[derive(Subcommand)]
pub enum FundingCommand {
    /// Lists the funding sources that sessions can run on, in order of preference, as JSON: the
    /// `env` to set for a session on each, and its threshold
    List {
        #[arg(long, value_enum)]
        tool: Option<ToolArg>,
    },
    /// Picks the source for a new session as JSON (`state` `ok`, `unknown`, `overThreshold`, or
    /// `none`, with the `source` and `env` unless `none`). Exits with 3 for `none`
    Pick {
        #[arg(long, value_enum)]
        tool: ToolArg,
    },
}

/// Prints the answer. Whether the answer is `none`, for the exit status.
pub fn run(data: &DataDir, command: &FundingCommand) -> Result<bool> {
    let config = data.load_config().context("cannot read config.json")?;
    match command {
        FundingCommand::List { tool } => {
            let listed = list(&config.funding, tool.map(Tool::from)).map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&listed)?);
            Ok(false)
        }
        FundingCommand::Pick { tool } => {
            let usage = UsageFiles::new(data.root().to_owned());
            let now = OffsetDateTime::now_utc();
            let picked =
                pick(&config.funding, &usage, (*tool).into(), now).map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&picked)?);
            Ok(picked.state == PickState::None)
        }
    }
}
