//! Collects ordered, unique search texts within ASIX and document unit limits.

use std::collections::BTreeSet;

use crate::{Policy, Result};

pub(crate) struct Texts<'a> {
    policy: &'a Policy,
    seen: BTreeSet<String>,
    values: Vec<String>,
    units: usize,
}

impl<'a> Texts<'a> {
    pub fn new(policy: &'a Policy) -> Self {
        Self {
            policy,
            seen: BTreeSet::new(),
            values: Vec::new(),
            units: 0,
        }
    }

    /// Returns whether the value added a text, so duplicate forms spend no synonym quota.
    pub fn push(&mut self, value: &str) -> Result<bool> {
        if value.trim().is_empty() || self.seen.contains(value) {
            return Ok(false);
        }

        let length = value.encode_utf16().count();
        let units = self
            .units
            .checked_add(length)
            .ok_or("document unit count overflow")?;
        if length > 65535 || Policy::exceeds(self.policy.max_document_units, units) {
            return Err("question exceeds analysis UTF-16 budget".into());
        }

        self.seen.insert(value.to_owned());
        self.values.push(value.to_owned());
        self.units = units;
        Ok(true)
    }

    pub fn into_vec(self) -> Vec<String> {
        self.values
    }
}
