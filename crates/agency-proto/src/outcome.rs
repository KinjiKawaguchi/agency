//! What an org measures and what its teams aim at (docs/protocol.md, "Org definition"). Only labels and ids:
//! the numbers are business data that stay on the owner's node, so the signed definition, which every node
//! copies, holds none, and no target figure either.
use crate::ids::{Id, IdError};
use crate::org::{label_ok, unique, OrgDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// The most KPIs an org states, and the most aims a team's plan holds.
pub const MAX_KPIS: usize = 100;
pub const MAX_AIMS: usize = 50;
/// An epic's item reference in an aim, in characters.
pub const EPIC: usize = 200;

/// An ISO 4217 currency code: three uppercase ASCII letters (`JPY`, `USD`). Its shape is checked, not its
/// presence in the list of codes in use, which changes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, type = "string")]
pub struct Currency(String);

impl Currency {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let is_code = raw.len() == 3 && raw.bytes().all(|b| b.is_ascii_uppercase());
        if is_code {
            Ok(Self(raw.to_owned()))
        } else {
            Err(IdError(format!(
                "not an ISO 4217 currency code (three uppercase letters): {raw}"
            )))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Currency {
    type Error = IdError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<Currency> for String {
    fn from(currency: Currency) -> Self {
        currency.0
    }
}

/// Whether a KPI is about the business or about the product.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KpiKind {
    Business,
    Product,
}

/// Which way is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GoodWhen {
    Higher,
    Lower,
}

/// What a KPI's number counts. A number is any finite value (negative ones included) and no range is checked for any
/// unit: `money` is an amount in its currency, never converted; `percent` is a number of percent (41.5 means
/// 41.5%, and it can be above 100); `count` is a plain count; `ratio` is a plain number (1.5 means 1.5 times); and
/// `seconds` is a duration in seconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "unit",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum KpiUnit {
    Money { currency: Currency },
    Percent,
    Count,
    Ratio,
    Seconds,
}

/// A measure the org states. It holds no number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Kpi {
    pub id: Id,
    pub label: String,
    pub kind: KpiKind,
    #[serde(flatten)]
    #[ts(flatten)]
    pub unit: KpiUnit,
    pub good_when: GoodWhen,
}

/// What a team's plan says an epic is meant to move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Aim {
    /// The epic's item reference.
    pub epic: String,
    pub kpi: Id,
}

fn is_epic_reference(raw: &str) -> bool {
    (1..=EPIC).contains(&raw.chars().count()) && !raw.chars().any(char::is_control)
}

/// The rules for what an org measures: checked by `OrgDefinition::validate`, so every node refuses a bad
/// version the same way. The KPIs have unique ids, well-formed labels, and are at most `MAX_KPIS`; a team's
/// aims are at most `MAX_AIMS`, name a KPI the definition lists, and are not repeated.
pub(crate) fn validate(definition: &OrgDefinition) -> Result<(), String> {
    if definition.kpis.len() > MAX_KPIS {
        return Err(format!("an org states at most {MAX_KPIS} KPIs"));
    }
    if !unique(definition.kpis.iter().map(|kpi| &kpi.id)) {
        return Err("a KPI appears twice".into());
    }
    if !definition.kpis.iter().all(|kpi| label_ok(&kpi.label)) {
        return Err("malformed KPI label".into());
    }
    let known: HashSet<&Id> = definition.kpis.iter().map(|kpi| &kpi.id).collect();
    for team in &definition.teams {
        let aims = team.plan.as_ref().map_or(&[][..], |plan| &plan.aims[..]);
        if aims.len() > MAX_AIMS {
            return Err(format!("a team's plan has at most {MAX_AIMS} aims"));
        }
        let mut seen = HashSet::new();
        let is_ok = aims.iter().all(|aim| {
            is_epic_reference(&aim.epic)
                && known.contains(&aim.kpi)
                && seen.insert((&aim.epic, &aim.kpi))
        });
        if !is_ok {
            return Err(
                "an aim names an epic that is not a reference, a KPI the org does not state, or repeats another"
                    .into(),
            );
        }
    }
    Ok(())
}
