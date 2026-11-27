//! The command's answer: the budget status as the one line it prints.
use agency_core::budget::BudgetStatus;
use anyhow::Result;

/// Serializes the complete command answer as its only stdout line.
pub fn json_line(status: &BudgetStatus) -> Result<String> {
    Ok(serde_json::to_string(status)?)
}
