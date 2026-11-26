//! `agency dept-add`, `dept-set`, `dept-remove`, `team-dept`, and `team-plan`: the org's departments,
//! the department each team is in, and what the org intends for each team, changed on the root key's
//! node.
use agency_core::departments::{self, Change};
use agency_core::Node;
use agency_proto::org::{Allocation, Department, SignedOrg, TeamPlan};
use agency_proto::Id;
use anyhow::Result;
use clap::Args;

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

fn optional(raw: Option<&str>) -> Result<Option<Id>> {
    raw.map(id).transpose()
}

/// Adds a department to an org (on the root key's node)
#[derive(Args)]
pub struct DeptAdd {
    #[arg(long)]
    org: String,
    /// The department's id
    #[arg(long)]
    dept: String,
    #[arg(long)]
    label: String,
    /// The department it is under
    #[arg(long)]
    parent: Option<String>,
    /// The member who heads it (a member id)
    #[arg(long)]
    head: Option<String>,
}

/// Changes a department's label, parent, or head (on the root key's node)
#[derive(Args)]
pub struct DeptSet {
    #[arg(long)]
    org: String,
    #[arg(long)]
    dept: String,
    #[arg(long)]
    label: Option<String>,
    /// The department it is under
    #[arg(long, conflicts_with = "no_parent")]
    parent: Option<String>,
    /// Puts it at the top, under no department
    #[arg(long)]
    no_parent: bool,
    /// The member who heads it (a member id)
    #[arg(long, conflicts_with = "no_head")]
    head: Option<String>,
    /// Leaves it without a head
    #[arg(long)]
    no_head: bool,
}

/// Removes a department that holds no department, member, or team (on the root key's node)
#[derive(Args)]
pub struct DeptRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    dept: String,
}

/// Puts a team in a department, or with --none in none (on the root key's node)
#[derive(Args)]
pub struct TeamDept {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    #[arg(long, required_unless_present = "none", conflicts_with = "none")]
    dept: Option<String>,
    #[arg(long)]
    none: bool,
}

fn report(what: String, doc: &SignedOrg) {
    println!("{what} (version {})", doc.body.version);
}

fn cleared_or(is_cleared: bool, value: Option<&str>) -> Result<Option<Option<Id>>> {
    if is_cleared {
        return Ok(Some(None));
    }
    Ok(optional(value)?.map(Some))
}

pub fn dept_add(node: &Node, args: DeptAdd) -> Result<()> {
    let department = Department {
        id: id(&args.dept)?,
        label: args.label,
        parent: optional(args.parent.as_deref())?,
        head: optional(args.head.as_deref())?,
    };
    let doc = departments::add(node, &id(&args.org)?, department)?;
    report(
        format!("Added department {} to org {}", args.dept, args.org),
        &doc,
    );
    Ok(())
}

pub fn dept_set(node: &Node, args: DeptSet) -> Result<()> {
    let change = Change {
        label: args.label,
        parent: cleared_or(args.no_parent, args.parent.as_deref())?,
        head: cleared_or(args.no_head, args.head.as_deref())?,
    };
    let doc = departments::change(node, &id(&args.org)?, &id(&args.dept)?, change)?;
    report(
        format!("Changed department {} of org {}", args.dept, args.org),
        &doc,
    );
    Ok(())
}

pub fn dept_remove(node: &Node, args: DeptRemove) -> Result<()> {
    let doc = departments::remove(node, &id(&args.org)?, &id(&args.dept)?)?;
    report(
        format!("Removed department {} from org {}", args.dept, args.org),
        &doc,
    );
    Ok(())
}

pub fn team_dept(node: &Node, args: TeamDept) -> Result<()> {
    let department = optional(args.dept.as_deref())?;
    let doc = departments::place_team(node, &id(&args.org)?, &id(&args.team)?, department)?;
    let place = args.dept.as_deref().map_or_else(
        || "in no department".to_owned(),
        |dept| format!("in department {dept}"),
    );
    report(
        format!("Team {} of org {} is now {place}", args.team, args.org),
        &doc,
    );
    Ok(())
}

/// The lines `agency status` shows for an org's departments, parents first.
pub fn department_lines(body: &agency_proto::OrgDefinition) -> Vec<String> {
    body.departments
        .iter()
        .map(|department| {
            let teams: Vec<&str> = body
                .teams
                .iter()
                .filter(|team| team.department.as_ref() == Some(&department.id))
                .map(|team| team.id.as_str())
                .collect();
            format!(
                "  Department {} ({}), under {}, head {}, teams: {}",
                department.id,
                department.label,
                department.parent.as_ref().map_or("none", Id::as_str),
                department.head.as_ref().map_or("none", Id::as_str),
                if teams.is_empty() {
                    "none".to_owned()
                } else {
                    teams.join(", ")
                }
            )
        })
        .collect()
}

/// Sets what the org intends for a team: the percent of its effort for each kind of work (adding up to
/// 100) and its weekly budget in whole US dollars; or with --none clears it (on the root key's node)
#[derive(Args)]
pub struct TeamPlanArgs {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    #[arg(long, required_unless_present = "none", conflicts_with = "none")]
    new_feature: Option<u32>,
    #[arg(long, required_unless_present = "none")]
    improvement: Option<u32>,
    #[arg(long, required_unless_present = "none")]
    maintenance: Option<u32>,
    #[arg(long, required_unless_present = "none")]
    debt: Option<u32>,
    #[arg(long, required_unless_present = "none")]
    incident: Option<u32>,
    /// The weekly spend the org intends, in whole US dollars at API prices
    #[arg(long, required_unless_present = "none")]
    weekly_budget_usd: Option<u32>,
    #[arg(long)]
    none: bool,
}

fn plan_of(args: &TeamPlanArgs) -> Option<TeamPlan> {
    Some(TeamPlan {
        aims: vec![],
        allocation: Allocation {
            new_feature: args.new_feature?,
            improvement: args.improvement?,
            maintenance: args.maintenance?,
            debt: args.debt?,
            incident: args.incident?,
        },
        weekly_budget_usd: args.weekly_budget_usd?,
    })
}

pub fn team_plan(node: &Node, args: TeamPlanArgs) -> Result<()> {
    let plan = if args.none { None } else { plan_of(&args) };
    let doc = departments::plan_team(node, &id(&args.org)?, &id(&args.team)?, plan)?;
    let what = if args.none { "no plan" } else { "a plan" };
    report(
        format!("Team {} of org {} now has {what}", args.team, args.org),
        &doc,
    );
    Ok(())
}

/// The lines `agency status` shows for the teams that have a plan.
pub fn plan_lines(body: &agency_proto::OrgDefinition) -> Vec<String> {
    let planned = body
        .teams
        .iter()
        .filter_map(|team| team.plan.as_ref().map(|plan| (team, plan)));
    planned
        .map(|(team, plan)| {
            let share = &plan.allocation;
            format!(
                "  Plan for team {}: new features {}%, improvement {}%, maintenance {}%, debt {}%, incidents {}%, {} USD a week",
                team.id,
                share.new_feature,
                share.improvement,
                share.maintenance,
                share.debt,
                share.incident,
                plan.weekly_budget_usd
            )
        })
        .collect()
}
