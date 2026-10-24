//! Converts upstream text resources into a deterministic, Rust-owned representation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::Result;

const FORMAT: &str = "asearch-analysis-resources/1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Synonym {
    pub text: String,
    pub trigger: bool,
}

/// Build-time language resources; these are never embedded into ASIX or WASM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub format: String,
    pub stopwords: BTreeSet<String>,
    pub stoptags: BTreeSet<Vec<String>>,
    pub groups: BTreeMap<String, Vec<Synonym>>,
}

fn lines(source: &str) -> impl Iterator<Item = &str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

impl Resources {
    /// Preserves CSV order and expansion direction while dropping comments and unused columns.
    pub fn compile(stopwords: &str, stoptags: &str, synonyms: &str) -> Result<Self> {
        let mut groups: BTreeMap<String, Vec<Synonym>> = BTreeMap::new();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(synonyms.as_bytes());
        for row in reader.records() {
            let row = row?;
            if row.len() != 11
                || row[0].len() != 6
                || !row[0].bytes().all(|c| c.is_ascii_digit())
                || !matches!(&row[2], "" | "0" | "1" | "2")
                || row[8].trim().is_empty()
            {
                return Err("invalid Sudachi synonym row".into());
            }
            if &row[2] == "2" {
                continue;
            }
            let entries = groups.entry(row[0].to_owned()).or_default();
            let trigger = matches!(&row[2], "" | "0");
            if let Some(entry) = entries.iter_mut().find(|entry| entry.text == row[8]) {
                entry.trigger |= trigger;
            } else {
                entries.push(Synonym {
                    text: row[8].to_owned(),
                    trigger,
                });
            }
        }
        let result = Self {
            format: FORMAT.to_owned(),
            stopwords: lines(stopwords).map(str::to_lowercase).collect(),
            stoptags: lines(stoptags)
                .map(|line| line.split(',').map(str::to_owned).collect())
                .collect(),
            groups,
        };
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<()> {
        if self.format != FORMAT
            || self.stopwords.iter().any(String::is_empty)
            || self
                .stoptags
                .iter()
                .any(|tag| tag.is_empty() || tag.iter().any(String::is_empty))
            || self
                .groups
                .values()
                .flatten()
                .any(|entry| entry.text.trim().is_empty())
        {
            return Err("invalid compiled analysis resources".into());
        }
        Ok(())
    }
}

/// Retains group identity instead of copying each group's alternatives per trigger.
pub(crate) struct Lookup {
    triggers: BTreeMap<String, Vec<usize>>,
    groups: Vec<Vec<String>>,
}

impl Lookup {
    pub fn new(resources: &Resources) -> Self {
        let mut triggers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut groups = Vec::new();
        for entries in resources.groups.values() {
            let id = groups.len();
            for entry in entries.iter().filter(|entry| entry.trigger) {
                let ids = triggers.entry(entry.text.to_lowercase()).or_default();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            groups.push(entries.iter().map(|entry| entry.text.clone()).collect());
        }
        Self { triggers, groups }
    }

    /// Expands only original token forms, so output-only and transitive paths stay disabled.
    pub fn alternatives<'a>(&'a self, forms: &[&str]) -> Vec<&'a str> {
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        for form in forms {
            if let Some(groups) = self.triggers.get(&form.to_lowercase()) {
                for &id in groups {
                    for text in &self.groups[id] {
                        if seen.insert(text.as_str()) {
                            result.push(text.as_str());
                        }
                    }
                }
            }
        }
        result
    }
}
