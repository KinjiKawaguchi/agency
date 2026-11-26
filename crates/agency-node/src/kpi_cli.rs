//! `agency kpi-add`, `kpi-set`, `kpi-remove`, `plan-aim`, and `plan-unaim`: what the org measures and what
//! a team's plan aims at, changed on the root key's node. Labels and ids only, never a number.
use agency_core::kpis::{self, Change};
use agency_core::Node;
use agency_proto::org::SignedOrg;
use agency_proto::outcome::{Currency, GoodWhen, Kpi, KpiKind, KpiUnit};
use agency_proto::{Id, OrgDefinition};
use anyhow::{bail, Result};
use clap::{Args, ValueEnum};

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Kind {
    Business,
    Product,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Unit {
    Money,
    Percent,
    Count,
    Ratio,
    Seconds,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Better {
    Higher,
    Lower,
}

impl From<Kind> for KpiKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Business => Self::Business,
            Kind::Product => Self::Product,
        }
    }
}

impl From<Better> for GoodWhen {
    fn from(better: Better) -> Self {
        match better {
            Better::Higher => Self::Higher,
            Better::Lower => Self::Lower,
        }
    }
}

/// A unit from `--unit` and `--currency`: `money` needs an ISO 4217 code, and no other unit takes one.
fn unit_of(unit: Unit, currency: Option<&str>) -> Result<KpiUnit> {
    match (unit, currency) {
        (Unit::Money, Some(code)) => Ok(KpiUnit::Money {
            currency: Currency::parse(code).map_err(anyhow::Error::msg)?,
        }),
        (Unit::Money, None) => bail!("--unit money needs --currency, an ISO 4217 code such as JPY"),
        (_, Some(_)) => bail!("--currency goes with --unit money only"),
        (Unit::Percent, None) => Ok(KpiUnit::Percent),
        (Unit::Count, None) => Ok(KpiUnit::Count),
        (Unit::Ratio, None) => Ok(KpiUnit::Ratio),
        (Unit::Seconds, None) => Ok(KpiUnit::Seconds),
    }
}

/// States a KPI of the org: a measure with no number (on the root key's node)
#[derive(Args)]
pub struct KpiAdd {
    #[arg(long)]
    org: String,
    /// The KPI's id
    #[arg(long)]
    kpi: String,
    #[arg(long)]
    label: String,
    #[arg(long, value_enum)]
    kind: Kind,
    #[arg(long, value_enum)]
    unit: Unit,
    /// The ISO 4217 code of a money KPI (JPY, USD, ...); money is never converted
    #[arg(long)]
    currency: Option<String>,
    /// Which way is better
    #[arg(long, value_enum)]
    good_when: Better,
}

/// Changes a KPI's label, unit, or direction (on the root key's node)
#[derive(Args)]
pub struct KpiSet {
    #[arg(long)]
    org: String,
    #[arg(long)]
    kpi: String,
    #[arg(long)]
    label: Option<String>,
    #[arg(long, value_enum)]
    unit: Option<Unit>,
    /// The ISO 4217 code, with `--unit money`
    #[arg(long)]
    currency: Option<String>,
    #[arg(long, value_enum)]
    good_when: Option<Better>,
}

/// Takes a KPI out of the definition; refused while a team's plan aims at it (on the root key's node)
#[derive(Args)]
pub struct KpiRemove {
    #[arg(long)]
    org: String,
    #[arg(long)]
    kpi: String,
}

/// States that a team's plan aims an epic at a KPI; the team needs a plan first (on the root key's node)
#[derive(Args)]
pub struct PlanAim {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    /// The epic's item reference
    #[arg(long)]
    epic: String,
    #[arg(long)]
    kpi: String,
}

/// Takes an aim out of a team's plan (on the root key's node)
#[derive(Args)]
pub struct PlanUnaim {
    #[arg(long)]
    org: String,
    #[arg(long)]
    team: String,
    #[arg(long)]
    epic: String,
    #[arg(long)]
    kpi: String,
}

fn report(what: String, doc: &SignedOrg) {
    println!("{what} (version {})", doc.body.version);
}

pub fn kpi_add(node: &Node, args: KpiAdd) -> Result<()> {
    let kpi = Kpi {
        id: id(&args.kpi)?,
        label: args.label,
        kind: args.kind.into(),
        unit: unit_of(args.unit, args.currency.as_deref())?,
        good_when: args.good_when.into(),
    };
    let doc = kpis::add(node, &id(&args.org)?, kpi)?;
    report(format!("Added KPI {} to org {}", args.kpi, args.org), &doc);
    Ok(())
}

pub fn kpi_set(node: &Node, args: KpiSet) -> Result<()> {
    let unit = match (args.unit, args.currency.as_deref()) {
        (Some(unit), currency) => Some(unit_of(unit, currency)?),
        (None, Some(_)) => bail!("--currency goes with --unit money"),
        (None, None) => None,
    };
    let change = Change {
        label: args.label,
        unit,
        good_when: args.good_when.map(Into::into),
    };
    let doc = kpis::change(node, &id(&args.org)?, &id(&args.kpi)?, change)?;
    report(
        format!("Changed KPI {} of org {}", args.kpi, args.org),
        &doc,
    );
    Ok(())
}

pub fn kpi_remove(node: &Node, args: KpiRemove) -> Result<()> {
    let doc = kpis::remove(node, &id(&args.org)?, &id(&args.kpi)?)?;
    report(
        format!("Removed KPI {} from org {}", args.kpi, args.org),
        &doc,
    );
    Ok(())
}

pub fn plan_aim(node: &Node, args: PlanAim) -> Result<()> {
    let doc = kpis::aim(
        node,
        &id(&args.org)?,
        &id(&args.team)?,
        args.epic.clone(),
        &id(&args.kpi)?,
    )?;
    report(
        format!(
            "Team {} of org {} now aims {} at {}",
            args.team, args.org, args.epic, args.kpi
        ),
        &doc,
    );
    Ok(())
}

pub fn plan_unaim(node: &Node, args: PlanUnaim) -> Result<()> {
    let doc = kpis::unaim(
        node,
        &id(&args.org)?,
        &id(&args.team)?,
        &args.epic,
        &id(&args.kpi)?,
    )?;
    report(
        format!(
            "Team {} of org {} no longer aims {} at {}",
            args.team, args.org, args.epic, args.kpi
        ),
        &doc,
    );
    Ok(())
}

fn unit_text(unit: &KpiUnit) -> String {
    match unit {
        KpiUnit::Money { currency } => format!("money {}", currency.as_str()),
        KpiUnit::Percent => "percent".into(),
        KpiUnit::Count => "count".into(),
        KpiUnit::Ratio => "ratio".into(),
        KpiUnit::Seconds => "seconds".into(),
    }
}

/// The lines `agency status` shows for an org's KPIs and for the aims of its teams' plans.
pub fn kpi_lines(body: &OrgDefinition) -> Vec<String> {
    let kpis = body.kpis.iter().map(|kpi| {
        let kind = match kpi.kind {
            KpiKind::Business => "business",
            KpiKind::Product => "product",
        };
        let better = match kpi.good_when {
            GoodWhen::Higher => "higher",
            GoodWhen::Lower => "lower",
        };
        format!(
            "  KPI {} ({}), {kind}, {}, {better} is better",
            kpi.id,
            kpi.label,
            unit_text(&kpi.unit)
        )
    });
    let aims = body.teams.iter().flat_map(|team| {
        let aims = team.plan.iter().flat_map(|plan| &plan.aims);
        aims.map(|aim| format!("  Team {} aims {} at KPI {}", team.id, aim.epic, aim.kpi))
            .collect::<Vec<_>>()
    });
    kpis.chain(aims).collect()
}
