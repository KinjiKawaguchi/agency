//! `agency seat-add`, `seat-set`, `seat-remove`, `seat-merge`, `external-add`, `external-set`, and `external-remove`: the
//! org's seats (places no one has taken yet) and external parties (people the org asks through a member), changed
//! on the root key's node. A `set` changes only the flags it is given.
use agency_core::seats::{self, ExternalChange, SeatChange};
use agency_core::Node;
use agency_proto::org::printable;
use agency_proto::org::SignedOrg;
use agency_proto::seat::{External, Seat};
use agency_proto::{Id, OrgDefinition};
use anyhow::Result;
use clap::Args;

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

fn ids(raw: &[String]) -> Result<Vec<Id>> {
    raw.iter().map(|one| id(one)).collect()
}

fn report(what: String, doc: &SignedOrg) {
    println!("{what} (version {})", doc.body.version);
}

/// Replaces a list with `given`, or empties it with `clear`, or leaves it (`None`).
fn replaced<T: Clone>(given: &[T], clear: bool) -> Option<Vec<T>> {
    if clear {
        return Some(vec![]);
    }
    (!given.is_empty()).then(|| given.to_vec())
}

/// Adds a seat: a place in the org that no one has taken yet (on the root key's node)
#[derive(Args)]
pub struct SeatAdd {
    #[arg(long)]
    org: String,
    /// The seat's id, which it keeps when someone takes it
    #[arg(long)]
    seat: String,
    #[arg(long)]
    label: String,
    /// The department it is in
    #[arg(long)]
    dept: Option<String>,
    /// What to ask this seat about; repeat it for each
    #[arg(long = "ask-about")]
    ask_about: Vec<String>,
    /// A member who answers for it; repeat it for each, in order of preference (none: the team's owner does)
    #[arg(long = "answered-by")]
    answered_by: Vec<String>,
}

/// Changes a seat; only the flags given change it (on the root key's node)
#[derive(Args)]
pub struct SeatSet {
    #[arg(long)]
    org: String,
    #[arg(long)]
    seat: String,
    #[arg(long)]
    label: Option<String>,
    /// The department it is in
    #[arg(long, conflicts_with = "no_dept")]
    dept: Option<String>,
    /// Puts it in no department
    #[arg(long)]
    no_dept: bool,
    /// What to ask it about, replacing the list; repeat it for each
    #[arg(long = "ask-about", conflicts_with = "clear_ask_about")]
    ask_about: Vec<String>,
    /// Asks it about nothing
    #[arg(long)]
    clear_ask_about: bool,
    /// The members who answer for it, replacing the list
    #[arg(long = "answered-by", conflicts_with = "clear_answered_by")]
    answered_by: Vec<String>,
    /// Leaves it to the team's owner to answer for it
    #[arg(long)]
    clear_answered_by: bool,
}

/// Removes a seat (on the root key's node)
#[derive(Args)]
pub struct SeatRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    seat: String,
}

/// Gives a seat to a member who is in the org already: the seat is gone, and its id and what to ask about it
/// are theirs (on the root key's node)
#[derive(Args)]
pub struct SeatMerge {
    #[arg(long)]
    org: String,
    #[arg(long)]
    seat: String,
    /// The member who takes it
    #[arg(long)]
    into: String,
}

/// Adds an external party: someone the org asks through a member and who never takes a seat (on the root
/// key's node)
#[derive(Args)]
pub struct ExternalAdd {
    #[arg(long)]
    org: String,
    #[arg(long)]
    external: String,
    #[arg(long)]
    label: String,
    /// What to ask them about; repeat it for each
    #[arg(long = "ask-about")]
    ask_about: Vec<String>,
    /// A member who answers for them; repeat it for each, in order of preference
    #[arg(long = "answered-by", required = true)]
    answered_by: Vec<String>,
}

/// Changes an external party; only the flags given change them (on the root key's node)
#[derive(Args)]
pub struct ExternalSet {
    #[arg(long)]
    org: String,
    #[arg(long)]
    external: String,
    #[arg(long)]
    label: Option<String>,
    /// What to ask them about, replacing the list; repeat it for each
    #[arg(long = "ask-about", conflicts_with = "clear_ask_about")]
    ask_about: Vec<String>,
    /// Asks them about nothing
    #[arg(long)]
    clear_ask_about: bool,
    /// The members who answer for them, replacing the list (at least one)
    #[arg(long = "answered-by")]
    answered_by: Vec<String>,
}

/// Removes an external party (on the root key's node)
#[derive(Args)]
pub struct ExternalRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    external: String,
}

pub fn seat_add(node: &Node, args: SeatAdd) -> Result<()> {
    let seat = Seat {
        id: id(&args.seat)?,
        label: args.label,
        department: args.dept.as_deref().map(id).transpose()?,
        unit: None,
        ask_about: args.ask_about,
        relays: ids(&args.answered_by)?,
    };
    let doc = seats::add_seat(node, &id(&args.org)?, seat)?;
    report(
        format!("Added seat {} to org {}", args.seat, args.org),
        &doc,
    );
    Ok(())
}

pub fn seat_set(node: &Node, args: SeatSet) -> Result<()> {
    let department = match (args.no_dept, args.dept.as_deref()) {
        (true, _) => Some(None),
        (false, given) => given.map(id).transpose()?.map(Some),
    };
    let change = SeatChange {
        label: args.label,
        department,
        ask_about: replaced(&args.ask_about, args.clear_ask_about),
        relays: replaced(&ids(&args.answered_by)?, args.clear_answered_by),
    };
    let doc = seats::change_seat(node, &id(&args.org)?, &id(&args.seat)?, change)?;
    report(
        format!("Changed seat {} of org {}", args.seat, args.org),
        &doc,
    );
    Ok(())
}

pub fn seat_remove(node: &Node, args: SeatRemove) -> Result<()> {
    let doc = seats::remove_seat(node, &id(&args.org)?, &id(&args.seat)?)?;
    report(
        format!("Removed seat {} from org {}", args.seat, args.org),
        &doc,
    );
    Ok(())
}

pub fn seat_merge(node: &Node, args: SeatMerge) -> Result<()> {
    let doc = seats::merge_seat(node, &id(&args.org)?, &id(&args.seat)?, &id(&args.into)?)?;
    report(
        format!(
            "Gave seat {} of org {} to {}",
            args.seat, args.org, args.into
        ),
        &doc,
    );
    Ok(())
}

pub fn external_add(node: &Node, args: ExternalAdd) -> Result<()> {
    let external = External {
        id: id(&args.external)?,
        label: args.label,
        ask_about: args.ask_about,
        relays: ids(&args.answered_by)?,
    };
    let doc = seats::add_external(node, &id(&args.org)?, external)?;
    report(
        format!("Added external party {} to org {}", args.external, args.org),
        &doc,
    );
    Ok(())
}

pub fn external_set(node: &Node, args: ExternalSet) -> Result<()> {
    let change = ExternalChange {
        label: args.label,
        ask_about: replaced(&args.ask_about, args.clear_ask_about),
        relays: replaced(&ids(&args.answered_by)?, false),
    };
    let doc = seats::change_external(node, &id(&args.org)?, &id(&args.external)?, change)?;
    report(
        format!(
            "Changed external party {} of org {}",
            args.external, args.org
        ),
        &doc,
    );
    Ok(())
}

pub fn external_remove(node: &Node, args: ExternalRemove) -> Result<()> {
    let doc = seats::remove_external(node, &id(&args.org)?, &id(&args.external)?)?;
    report(
        format!(
            "Removed external party {} from org {}",
            args.external, args.org
        ),
        &doc,
    );
    Ok(())
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

/// The lines `agency status` prints about an org's seats and external parties.
pub fn seat_lines(body: &OrgDefinition) -> Vec<String> {
    let names = |list: &[Id]| {
        list.iter()
            .map(|one| one.as_str().to_owned())
            .collect::<Vec<_>>()
    };
    let seats = body.seats.iter().map(|seat| {
        format!(
            "  Seat {} ({}), in {}, ask about: {}, answered by: {}",
            seat.id,
            printable(&seat.label),
            seat.department.as_ref().map_or("no department", Id::as_str),
            printable(&list_or_none(&seat.ask_about)),
            if seat.relays.is_empty() {
                "the team's owner".to_owned()
            } else {
                list_or_none(&names(&seat.relays))
            }
        )
    });
    let externals = body.externals.iter().map(|external| {
        format!(
            "  External party {} ({}), ask about: {}, answered by: {}",
            external.id,
            printable(&external.label),
            printable(&list_or_none(&external.ask_about)),
            list_or_none(&names(&external.relays))
        )
    });
    seats.chain(externals).collect()
}
