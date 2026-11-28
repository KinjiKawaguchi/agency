//! The settings of the numbers a KPI takes from the node itself: `outcomes` in `config.json`. The org's
//! definition holds no number and no source; which KPI reads which of the node's delivery numbers is the
//! operator's setting on the owner's node.
use crate::config::HostedTeam;
use agency_proto::outcome::KpiUnit;
use agency_proto::Id;
use serde::{Deserialize, Serialize};

/// How many connectors a node may have.
pub const MOST_CONNECTORS: usize = 50;

/// A delivery number of one day that a KPI can take as its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Metric {
    /// The new features merged that day: a count.
    NewFeaturesMerged,
    /// The median of opened to merged over the new features merged that day: seconds.
    NewFeaturePrLeadMedianSeconds,
    /// The seconds main's CI was red that day.
    MainRedSeconds,
    /// The day's cost over the pull requests merged that day: US dollars.
    CostPerMergedPrUsd,
    /// The median of opened to merged over every pull request merged that day: seconds.
    PrLeadMedianSeconds,
}

impl Metric {
    /// Whether a KPI in this unit can hold the metric.
    pub fn fits(self, unit: &KpiUnit) -> bool {
        match (self, unit) {
            (Self::NewFeaturesMerged, KpiUnit::Count) => true,
            (
                Self::NewFeaturePrLeadMedianSeconds
                | Self::MainRedSeconds
                | Self::PrLeadMedianSeconds,
                KpiUnit::Seconds,
            ) => true,
            (Self::CostPerMergedPrUsd, KpiUnit::Money { currency }) => currency.as_str() == "USD",
            _ => false,
        }
    }

    /// The unit a KPI holding this metric has, in words.
    pub fn unit_words(self) -> &'static str {
        match self {
            Self::NewFeaturesMerged => "count",
            Self::NewFeaturePrLeadMedianSeconds
            | Self::MainRedSeconds
            | Self::PrLeadMedianSeconds => "seconds",
            Self::CostPerMergedPrUsd => "money in USD",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::NewFeaturesMerged => "newFeaturesMerged",
            Self::NewFeaturePrLeadMedianSeconds => "newFeaturePrLeadMedianSeconds",
            Self::MainRedSeconds => "mainRedSeconds",
            Self::CostPerMergedPrUsd => "costPerMergedPrUsd",
            Self::PrLeadMedianSeconds => "prLeadMedianSeconds",
        }
    }
}

/// One KPI of an org and the delivery number of a team that runs on this node, which becomes its number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connector {
    pub org: Id,
    pub team: Id,
    pub kpi: Id,
    pub metric: Metric,
}

/// `outcomes` in `config.json`. The section can be left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct OutcomesConfig {
    /// The KPIs whose daily number the node writes itself, from its delivery numbers.
    pub connectors: Vec<Connector>,
}

impl OutcomesConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Why these settings cannot be used, in words that name the setting. `teams` are the teams the node
    /// runs.
    pub fn check(&self, teams: &[HostedTeam]) -> Result<(), String> {
        if self.connectors.len() > MOST_CONNECTORS {
            return Err(format!(
                "outcomes.connectors has more than {MOST_CONNECTORS} entries"
            ));
        }
        for (index, one) in self.connectors.iter().enumerate() {
            let team = teams
                .iter()
                .find(|team| team.org == one.org && team.team == one.team);
            let Some(team) = team else {
                return Err(format!(
                    "outcomes.connectors[{index}].team: {}/{} does not run on this node",
                    one.org, one.team
                ));
            };
            let has_repos = team
                .delivery
                .as_ref()
                .is_some_and(|delivery| !delivery.repos.is_empty());
            if !has_repos {
                return Err(format!(
                    "outcomes.connectors[{index}].team: {}/{} has no delivery repositories to take numbers from",
                    one.org, one.team
                ));
            }
            let first = self
                .connectors
                .iter()
                .position(|other| other.org == one.org && other.kpi == one.kpi);
            if first != Some(index) {
                return Err(format!(
                    "outcomes.connectors[{index}].kpi: {} is already set by outcomes.connectors[{}]",
                    one.kpi,
                    first.unwrap_or(0)
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_connector_names_the_org_team_kpi_and_one_metric_of_a_short_list() {
        let text = r#"{"connectors":[{"org":"acme","team":"app","kpi":"shipped","metric":"newFeaturesMerged"}]}"#;
        let config: OutcomesConfig = serde_json::from_str(text).unwrap();
        assert_eq!(config.connectors[0].metric, Metric::NewFeaturesMerged);
        assert_eq!(serde_json::to_string(&config).unwrap(), text);
        for metric in [
            "newFeaturesMerged",
            "newFeaturePrLeadMedianSeconds",
            "mainRedSeconds",
            "costPerMergedPrUsd",
            "prLeadMedianSeconds",
        ] {
            let one: Metric = serde_json::from_value(serde_json::json!(metric)).unwrap();
            assert_eq!(one.name(), metric);
        }
        for bad in [
            r#"{"connectors":[{"org":"acme","team":"app","kpi":"k","metric":"revenue"}]}"#,
            r#"{"connectors":[{"org":"acme","team":"app","kpi":"k","metric":"mainRedSeconds","token":"x"}]}"#,
            r#"{"connectors":[{"org":"acme","team":"app","metric":"mainRedSeconds"}]}"#,
        ] {
            assert!(
                serde_json::from_str::<OutcomesConfig>(bad).is_err(),
                "{bad}"
            );
        }
        assert!(OutcomesConfig::default().is_default());
    }
}
