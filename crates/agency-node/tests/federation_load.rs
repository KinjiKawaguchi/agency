//! A load test of federation (agency#529): how long one pass of a home node's background work takes as the number
//! of team nodes and teams grows, and what a team node that never answers costs. It is ignored by default; run it
//! with `scripts/load/federation.sh`, which prints one table row per configuration. `docs/scale.md` keeps the
//! numbers and the machine they came from.
//!
//! Every node runs in this process on the loopback, with memory runtimes, and is reached over HTTP as in
//! production. The home node is the org's root (`alice`); each other node joins the org and hosts `teams` teams.
//! A pass is what the node's loop does every 15 seconds: `refresh` (every remote team's view, each peer's
//! `about`, the definition), the push pass, and the outbox.
mod common;

use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::decisions::Decided;
use agency_core::devices::DeviceSigner;
use agency_core::invites::{invite, join, Joined};
use agency_core::orgs;
use agency_core::outbox::deliver_due;
use agency_core::push::notify_waiting;
use agency_core::signed_decisions::decide_with_device;
use agency_core::views::refresh;
use agency_proto::org::TeamPlace;
use agency_proto::Role;
use agency_proto::{EnvelopeKind, Payload};
use common::{alice, device, id, memory_runtime, node, Running};
use std::net::TcpListener;
use std::path::Path;
use std::time::{Duration, Instant};

/// Requests in every team's view, so that a view has the size of a busy team's.
const REQUESTS_PER_TEAM: usize = 20;

/// One configuration: team nodes besides the home node, teams on each, and teams on a node that never answers.
struct Shape {
    nodes: usize,
    teams: usize,
    silent_teams: usize,
}

/// What one configuration measured. Times are the median of the passes after the first.
struct Measured {
    remote_teams: usize,
    refresh: Duration,
    push: Duration,
    outbox: Duration,
    cpu: Option<Duration>,
    view_bytes: u64,
    decision: Duration,
    home_bytes: u64,
}

/// CPU time used by this process so far (user and system), on Linux; `None` elsewhere. Every node runs in this
/// process, so this counts the team nodes answering as well as the home node reading.
fn cpu_time() -> Option<Duration> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let after_name = &stat[stat.rfind(')')? + 2..];
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    let ticks: u64 = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    Some(Duration::from_millis(ticks * 10))
}

fn size_of(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return std::fs::metadata(path).map_or(0, |one| one.len());
    };
    entries.flatten().map(|entry| size_of(&entry.path())).sum()
}

/// A listener that takes connections and never answers: every request to it waits for the reader's timeout.
fn silent_node() -> (String, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let accepting = listener.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut held = vec![];
        for stream in accepting.incoming().flatten() {
            held.push(stream);
        }
    });
    (origin, listener)
}

/// The org with its team nodes: `alice` (root, the home node measured) and the others, each hosting its teams.
async fn org_of(shape: &Shape) -> (Running, Vec<Running>, Option<TcpListener>) {
    let (alice, _) = alice().await;
    let mut members = vec![];
    for index in 0..shape.nodes {
        let (runtime, memory) = memory_runtime();
        {
            let mut state = memory.lock().unwrap();
            for request in 0..REQUESTS_PER_TEAM {
                state.requests.push((
                    format!("q-{request}"),
                    format!("Question {request} about the release plan?"),
                ));
            }
        }
        let member = node(&format!("n{index}"), runtime).await;
        let link = invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
        let joined = join(&member.node, &link, |_| true).await.unwrap();
        assert!(matches!(joined, Joined::Member(_)), "{joined:?}");
        members.push(member);
    }
    let mut places = vec![];
    for (index, member) in members.iter().enumerate() {
        let teams: Vec<_> = (0..shape.teams)
            .map(|team| id(&format!("t{index}x{team}")))
            .collect();
        member
            .node
            .update_config(|config| {
                for team in &teams {
                    let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
                    config.teams.push(HostedTeam::new(
                        id("acme"),
                        team.clone(),
                        id("alice"),
                        runtime,
                    ));
                }
            })
            .unwrap();
        places.extend(
            teams
                .into_iter()
                .map(|team| TeamPlace::new(team.clone(), team.to_string(), member.url())),
        );
    }
    let silent = (shape.silent_teams > 0).then(silent_node);
    if let Some((origin, _)) = &silent {
        places.extend((0..shape.silent_teams).map(|team| {
            let team = id(&format!("s{team}"));
            TeamPlace::new(team.clone(), team.to_string(), origin.clone())
        }));
    }
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.teams.extend(places);
        Ok(())
    })
    .unwrap();
    // The version that places the teams reaches their nodes through the outbox, and a node that missed versions
    // since it joined catches up as its own loop would.
    deliver_due(&alice.node).await.unwrap();
    for member in &members {
        refresh(&member.node).await.unwrap();
    }
    (alice, members, silent.map(|(_, listener)| listener))
}

async fn timed<F: std::future::Future>(work: F) -> Duration {
    let started = Instant::now();
    work.await;
    started.elapsed()
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

async fn measure(shape: &Shape) -> Measured {
    let (alice, members, _silent) = org_of(shape).await;
    let (mut refreshes, mut pushes, mut outboxes) = (vec![], vec![], vec![]);
    let mut cpu = None;
    for pass in 0..4 {
        let before = cpu_time();
        let refreshed = timed(async { refresh(&alice.node).await.unwrap() }).await;
        let pushed = timed(async { notify_waiting(&alice.node).await.unwrap() }).await;
        let delivered = timed(async { deliver_due(&alice.node).await.unwrap() }).await;
        if pass == 0 {
            continue;
        }
        cpu = before.zip(cpu_time()).map(|(before, after)| after - before);
        refreshes.push(refreshed);
        pushes.push(pushed);
        outboxes.push(delivered);
    }
    let view_bytes = alice
        .node
        .store()
        .orgs()
        .iter()
        .flat_map(|doc| doc.body.teams.clone())
        .filter_map(|place| alice.node.store().load_replica(&id("acme"), &place.id))
        .filter_map(|replica| replica.view)
        .map(|view| serde_json::to_vec(&view).map_or(0, |bytes| bytes.len() as u64))
        .sum();
    let read = alice
        .node
        .store()
        .orgs()
        .iter()
        .flat_map(|doc| doc.body.teams.clone())
        .filter(|place| {
            let replica = alice.node.store().load_replica(&id("acme"), &place.id);
            replica.is_some_and(|replica| replica.view.is_some())
        })
        .count();
    assert_eq!(
        read,
        shape.nodes * shape.teams,
        "every answering team was read"
    );
    let key = device(&alice).await;
    // The version with the new device reaches the team nodes first, as it would before anyone decides with it.
    deliver_due(&alice.node).await.unwrap();
    for member in &members {
        refresh(&member.node).await.unwrap();
    }
    let signer = key.public().device_id();
    let team = id("t0x0");
    let decision = timed(async {
        let decided = decide_with_device(
            &alice.node,
            DeviceSigner {
                id: &signer,
                key: &key,
            },
            &id("acme"),
            &team,
            EnvelopeKind::Answer,
            "q-0".into(),
            Payload {
                text: "ok".into(),
                choice: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(decided, Decided::Delivered);
    })
    .await;
    drop(members);
    Measured {
        remote_teams: shape.nodes * shape.teams + shape.silent_teams,
        refresh: median(refreshes),
        push: median(pushes),
        outbox: median(outboxes),
        cpu,
        view_bytes,
        decision,
        home_bytes: size_of(&alice.data_dir()),
    }
}

fn row(shape: &Shape, measured: &Measured) -> String {
    let ms = |time: Duration| format!("{:.0}", time.as_secs_f64() * 1000.0);
    format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
        shape.nodes,
        shape.teams,
        shape.silent_teams,
        measured.remote_teams,
        ms(measured.refresh),
        ms(measured.push),
        ms(measured.outbox),
        measured.cpu.map_or("n/a".to_owned(), ms),
        measured.view_bytes / 1024,
        ms(measured.decision),
        measured.home_bytes / 1024,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "a load test: run it with scripts/load/federation.sh"]
async fn federation_load() {
    let shapes = [
        Shape {
            nodes: 2,
            teams: 10,
            silent_teams: 0,
        },
        Shape {
            nodes: 5,
            teams: 50,
            silent_teams: 0,
        },
        Shape {
            nodes: 10,
            teams: 200,
            silent_teams: 0,
        },
        Shape {
            nodes: 2,
            teams: 10,
            silent_teams: 5,
        },
    ];
    println!("| team nodes | teams each | silent teams | remote teams | refresh ms | push ms | outbox ms | CPU ms per pass | views KiB per pass | decision ms | home data KiB |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for shape in &shapes {
        let measured = measure(shape).await;
        println!("{}", row(shape, &measured));
    }
}
