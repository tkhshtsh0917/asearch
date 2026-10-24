//! Validates application-level expansion limits separately from ASIX hard limits.

use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Selects token granularity and optional budgets; -1 disables an application budget.
pub struct Policy {
    pub split_mode: String,
    pub keep_part_of_speech: Vec<String>,
    pub max_synonym_texts: i64,
    pub max_document_units: i64,
}

impl Policy {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.split_mode.as_str(), "A" | "B" | "C")
            || self.keep_part_of_speech.is_empty()
            || self.keep_part_of_speech.iter().any(String::is_empty)
            || [self.max_synonym_texts, self.max_document_units]
                .iter()
                .any(|&n| n != -1 && n <= 0)
        {
            return Err(
                "analysis limits require -1 or positive integers and a valid split mode".into(),
            );
        }
        Ok(())
    }

    pub(crate) fn exceeds(limit: i64, actual: usize) -> bool {
        limit != -1 && actual as u128 > limit as u128
    }
}
