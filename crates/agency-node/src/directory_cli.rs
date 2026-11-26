//! `agency directory`: who the org's agents may ask for what, read on this node (docs/protocol.md, "The
//! directory for agents"). With `--json` it is the `Directory` a team's harness reads.
use agency_core::directory::directory;
use agency_core::Node;
use agency_proto::directory::{Directory, DirectoryParty, RuntimeNames};
use agency_proto::org::printable;
use agency_proto::Id;
use anyhow::Result;
use clap::Args;

/// Prints the org's members, seats, and external parties, with what to ask each about and the names each has
/// in this node's teams (read-only; for a person, or with --json for a team's harness)
#[derive(Args)]
pub struct DirectoryArgs {
    #[arg(long)]
    org: String,
    /// Prints the `Directory` as JSON
    #[arg(long)]
    json: bool,
}

fn names(runtime_names: &[RuntimeNames]) -> String {
    if runtime_names.is_empty() {
        return "none".to_owned();
    }
    let teams: Vec<String> = runtime_names
        .iter()
        .map(|one| format!("{}: {}", one.team, one.names.join(", ")))
        .collect();
    teams.join("; ")
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "nothing".to_owned()
    } else {
        items.join(", ")
    }
}

fn party_line(kind: &str, party: &DirectoryParty) -> String {
    let answered_by: Vec<String> = party.answered_by.iter().map(Id::to_string).collect();
    format!(
        "  {kind} {} ({}), ask about: {}; answered by: {}; names in this node's teams: {}",
        party.id,
        printable(&party.label),
        printable(&list(&party.ask_about)),
        if answered_by.is_empty() {
            "the team's owner".to_owned()
        } else {
            answered_by.join(", ")
        },
        names(&party.runtime_names)
    )
}

/// The directory in words, a line for each member, seat, and external party.
pub fn directory_lines(directory: &Directory) -> Vec<String> {
    let mut lines = vec![format!("Org {}:", directory.org)];
    for member in &directory.members {
        let ask_about = if member.ask_about.is_empty() {
            String::new()
        } else {
            format!(", ask about: {}", list(&member.ask_about))
        };
        lines.push(format!(
            "  Member {} ({}), {}{ask_about}, names in this node's teams: {}",
            member.id,
            printable(&member.label),
            member.role,
            names(&member.runtime_names)
        ));
    }
    lines.extend(directory.seats.iter().map(|one| party_line("Seat", one)));
    lines.extend(
        directory
            .externals
            .iter()
            .map(|one| party_line("External party", one)),
    );
    lines
}

pub fn run(node: &Node, args: DirectoryArgs) -> Result<()> {
    let org = Id::parse(&args.org).map_err(anyhow::Error::msg)?;
    let found = directory(node, &org)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&found)?);
    } else {
        for line in directory_lines(&found) {
            println!("{line}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::directory::DirectoryMember;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    #[test]
    fn each_member_seat_and_external_party_is_a_line_with_the_names_it_has() {
        let names = |team: &str, names: &[&str]| RuntimeNames {
            team: id(team),
            names: names.iter().map(|one| (*one).to_owned()).collect(),
        };
        let directory = Directory {
            org: id("acme"),
            departments: vec![],
            members: vec![
                DirectoryMember {
                    id: id("alice"),
                    label: "Alice".into(),
                    department: None,
                    role: "member".into(),
                    ask_about: vec![],
                    runtime_names: vec![names("app", &["owner", "reviewer"])],
                },
                DirectoryMember {
                    id: id("dave"),
                    label: "Dave".into(),
                    department: None,
                    role: "member".into(),
                    ask_about: vec!["invoices".into(), "budget approval".into()],
                    runtime_names: vec![],
                },
            ],
            seats: vec![DirectoryParty {
                id: id("carol"),
                label: "Carol (finance)".into(),
                department: None,
                ask_about: vec!["invoices".into()],
                answered_by: vec![],
                runtime_names: vec![],
            }],
            externals: vec![],
        };
        assert_eq!(
            directory_lines(&directory),
            [
                "Org acme:",
                "  Member alice (Alice), member, names in this node's teams: app: owner, reviewer",
                "  Member dave (Dave), member, ask about: invoices, budget approval, names in this node's teams: none",
                "  Seat carol (Carol (finance)), ask about: invoices; answered by: the team's owner; names in this node's teams: none",
            ]
        );
    }
}
