//! `agency`: Agency のノードの CLI。台を作り、組織を作り、招待し、参加し、動かす。
use agency_node::client::{decide, join, refresh, Decided};
use agency_node::config::{Account, HostedTeam, NodeConfig, RuntimeCommand};
use agency_node::node::Node;
use agency_node::orgs;
use agency_node::outbox::{deliver_due, pending};
use agency_node::server::router;
use agency_node::store::DataDir;
use agency_proto::org::{is_node_origin, TeamPlace};
use agency_proto::{EnvelopeKind, Id, Payload, Role};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "agency",
    about = "Agency のノード: AI エージェントと人の組織を、複数の機械にまたがって動かす"
)]
struct Cli {
    /// データのディレクトリ (既定は $AGENCY_DIR か ~/.agency)
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// この台を作る (アカウントの鍵を作り、設定を書く)
    Init {
        #[arg(long)]
        account: String,
        #[arg(long)]
        label: String,
        /// ほかの台からの届け先 (http://127.0.0.1:4400 など)
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "127.0.0.1:4400")]
        listen: String,
        /// ハーネスの CLI (scrum) のパス
        #[arg(long, default_value = "scrum")]
        scrum: PathBuf,
    },
    /// 組織を作る (この台のアカウントが根の鍵になる)
    OrgCreate {
        #[arg(long)]
        org: String,
        #[arg(long)]
        label: String,
    },
    /// 組織にチームを足す (根の鍵の台で)。--node を省くと、この台に置く
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
    /// 組織への招待のリンクを作る (根の鍵の台で)
    Invite {
        #[arg(long)]
        org: String,
        #[arg(long)]
        department: Option<String>,
        /// member (読む、答える) か answerer (答えるだけ)
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// 招待のリンクから組織に参加する
    Join {
        link: String,
        /// 確かめずに参加する
        #[arg(long)]
        yes: bool,
    },
    /// 要求に答える (この台のチームなら当て、ほかの台のチームなら署名して届ける)
    Decide {
        #[arg(long)]
        org: String,
        #[arg(long)]
        team: String,
        /// 要求の id
        #[arg(long)]
        request: String,
        #[arg(long)]
        choice: Option<String>,
        /// 答えの文 (- で始まる文もそのまま受け取る)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        text: String,
    },
    /// 写しを 1 回読み直し、送り箱を 1 回届け直す
    Sync,
    /// この台の様子 (アカウント、組織、置いているチーム、送り箱)
    Status,
    /// 台を動かす (台どうしの道を待ち受け、写しと送り箱を回す)
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
        bail!("{} はもうある", data.config_path().display());
    }
    if !is_node_origin(url) {
        bail!("--url は http(s)://ホスト[:ポート] の形");
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
    })?;
    println!(
        "台を作った: {}\nアカウント {account} の鍵の指紋: {}",
        data.root().display(),
        key.public().fingerprint()
    );
    Ok(())
}

fn status(node: &Node) {
    let config = node.config();
    println!(
        "アカウント: {} ({}) 鍵の指紋 {}",
        config.account.id,
        config.account.label,
        node.key.public().fingerprint()
    );
    println!("この台: {} (待ち受け {})", config.url, config.listen);
    for doc in node.data.orgs() {
        let role = if orgs::is_root(node, &doc) {
            "根の鍵"
        } else if doc.body.member(&config.account.id).is_some() {
            "所属"
        } else {
            "写し"
        };
        println!(
            "組織 {} ({}) {} 版、{}、人 {} 人、チーム {}",
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
            "送り箱: {} → {} ({} 回、{})",
            parcel.id,
            parcel.to,
            parcel.attempts,
            parcel.last_error.unwrap_or_default()
        );
    }
}

fn confirm_join(definition: &agency_proto::OrgDefinition) -> bool {
    print!(
        "組織「{}」({}) に参加しますか。根の鍵 {} の指紋はリンクと合っています [y/N] ",
        definition.label, definition.org, definition.root.account
    );
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
}

async fn serve(node: Arc<Node>) -> Result<()> {
    let listen = node.config().listen;
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .with_context(|| format!("{listen} で待ち受けられない"))?;
    println!("台を動かした: {} (待ち受け {listen})", node.url());
    let background = node.clone();
    tokio::spawn(async move {
        let mut tick = 0u64;
        loop {
            if tick.is_multiple_of(2) {
                if let Err(error) = refresh(&background).await {
                    eprintln!("agency: 写しの読み直しに失敗した: {error}");
                }
            }
            if let Err(error) = deliver_due(&background).await {
                eprintln!("agency: 送り箱の届け直しに失敗した: {error}");
            }
            tick += 1;
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
    axum::serve(listener, router(node))
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
    let node = Node::open(data, None)?;
    match cli.command {
        Command::Init { .. } => unreachable!(),
        Command::OrgCreate { org, label } => {
            let doc = orgs::create(&node, id(&org)?, label)?;
            println!(
                "組織 {} を作った。根の鍵の指紋: {}",
                doc.body.org,
                node.key.public().fingerprint()
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
                bail!("--node は http(s)://ホスト[:ポート] の形");
            }
            let is_here = place_url == node.url();
            if is_here && state_dir.is_none() {
                bail!("この台に置くチームは --state-dir が要る");
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
                "組織 {org} にチーム {team} を足した ({} 版)",
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
                _ => bail!("--role は member か answerer"),
            };
            let department = department.as_deref().map(id).transpose()?;
            println!("{}", orgs::invite(&node, &id(&org)?, department, role)?);
        }
        Command::Join { link, yes } => {
            let doc = join(&node, &link, |definition| yes || confirm_join(definition)).await?;
            println!(
                "組織 {} ({}) に参加した ({} 版)",
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
                    Decided::Applied => "この台のチームに当てた".to_owned(),
                    Decided::Delivered => "署名して、チームの台に届けた".to_owned(),
                    Decided::Queued(why) =>
                        format!("チームの台に届かないので、送り箱にため置いた ({why})"),
                    Decided::Refused(why) => format!("チームの台が断った: {why}"),
                }
            );
        }
        Command::Sync => {
            refresh(&node).await?;
            let outcomes = deliver_due(&node).await?;
            println!("写しを読み直し、送り箱を {} 件届けに行った", outcomes.len());
        }
        Command::Status => status(&node),
        Command::Serve => serve(node).await?,
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
