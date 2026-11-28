//! `agency status` and `sync`: what this node is and holds, and one pass of what it does in the background;
//! and the warnings about teams placed elsewhere, which `serve` also writes to the log.
use crate::{dept_cli, kpi_cli, peer_cli, seat_cli, seen_cli};
use agency_adapters::files::DataDir;
use agency_core::invites::{finish_joins, Joined};
use agency_core::outbox::{deliver_due, pending};
use agency_core::{orgs, Node};
use anyhow::Result;

pub fn status(node: &Node, data: &DataDir) {
    let releases = agency_adapters::runtime_releases::read(data);
    let config = node.config();
    println!(
        "Account: {} ({}), key fingerprint {}",
        config.account.id,
        config.account.label,
        node.key().public().fingerprint()
    );
    println!("This node: {} (listening on {})", config.url, config.listen);
    for line in agency_core::views::waiting_lines(node) {
        println!("{line}");
    }
    for line in agency_core::read_auth::status_lines(node) {
        println!("{line}");
    }
    if let Err(error) = node.store().own_origins() {
        println!("{}", agency_core::node::own_origins_unreadable_line(&error));
    }
    let waiting = agency_core::claims::pending(node).len();
    if waiting > 0 {
        println!("Claims waiting for you: {waiting} (agency claims)");
    }
    if !config.local_clients {
        println!("Local clients: refused (localClients is false); reach this node through federation or Access");
    }
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
            "Team on this node: {}/{} (owner {}) runs {}; {}",
            team.org,
            team.team,
            team.owner,
            team.runtime.command.join(" "),
            agency_adapters::runtime_releases::say(&releases, &team.org, &team.team)
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
pub fn warn_misplaced(node: &Node, write: impl Fn(String)) {
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
            misplaced.org,
            misplaced.team,
            node.url(),
            misplaced.org,
            misplaced.team,
            node.url()
        ));
    }
}

/// Says which entries of a team's `people` name no member, seat, or external party of its org, so that a
/// request addressed to one is not left for no one. It is a warning: a typo here must not stop a node.
pub fn warn_unknown_addressees(node: &Node, write: impl Fn(String)) {
    for unknown in orgs::unknown_addressees(node) {
        write(format!(
            "team {}/{}: the people entry \"{}\" names {}, who is not a member, seat, or external party of org {}, so a request addressed to {} is answered by no one",
            unknown.org, unknown.team, unknown.name, unknown.id, unknown.org, unknown.name
        ));
    }
}

/// Re-reads the replicas, asks again for held joins, and tries the outbox once.
pub async fn sync(node: &Node) -> Result<()> {
    agency_core::views::refresh_now(node).await?;
    for joined in finish_joins(node).await? {
        if let Joined::Member(doc) = joined {
            println!("Joined org {} (version {})", doc.body.org, doc.body.version);
        }
    }
    let outcomes = deliver_due(node).await?;
    println!(
        "Re-read the replicas and tried {} outbox deliveries",
        outcomes.len()
    );
    Ok(())
}
