//! Searches compiled intent DAGs and reconstructs a matching original expansion.

use super::{Entry, IntentHit, IntentIndex, IntentResults, matching};
use crate::{
    Explanation, QueryWorkspace, SearchError,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

impl IntentIndex {
    /// Ranks matching entries using one bounded DAG-by-pattern workspace.
    pub fn search(
        &self,
        query: &QueryWorkspace,
        casing: Option<&[u32]>,
        limit: usize,
        count_total: bool,
    ) -> Result<IntentResults, SearchError> {
        self.validate_query(query)?;
        let _query = self.reserve_working(query.capacity_bytes())?;
        let available = self.available_working_bytes()?;
        let _operation = self.reserve_working(available)?;
        let mut budget = Budget::new(size_of::<IntentResults>(), available)?;

        let (classes, leading) = crate::search::classes::compile(query, casing, &mut budget)?;
        let looping = leading || classes.iter().any(|class| class.looping);
        let possible = |entry: &&Entry| entry.possible(classes.len(), query.max_ambig(), looping);

        let max_nodes = self
            .entries
            .iter()
            .filter(possible)
            .map(|entry| entry.graph.nodes())
            .max()
            .unwrap_or(0);

        let mut cells = budget.reserve(mul(max_nodes, add(classes.len(), 1)?)?)?;
        cells.resize(cells.capacity(), 0);

        let mut levels = budget.reserve(self.entries.len())?;
        let mut totals = [0; 8];

        for entry in &self.entries {
            let level = if entry.possible(classes.len(), query.max_ambig(), looping) {
                matching::level(
                    &entry.graph,
                    &classes,
                    leading,
                    query.max_ambig() + 1,
                    &mut cells,
                )
            } else {
                query.max_ambig() + 1
            };
            levels.push(level);
            if level <= query.max_ambig() {
                totals[level as usize] += 1;
            }
        }

        let mut hits = budget.reserve(limit.min(self.entries.len()))?;
        'levels: for level in 0..=query.max_ambig() {
            for (position, entry) in self.entries.iter().enumerate() {
                if hits.len() >= limit {
                    break 'levels;
                }
                if levels[position] == level {
                    hits.push(IntentHit {
                        key: entry.key,
                        order: entry.order,
                        entry: position,
                        level: u16::from(level),
                    });
                }
            }
        }

        Ok(IntentResults {
            hits,
            totals: count_total.then_some(totals),
        })
    }

    /// Reconstructs one deterministic expansion and maps its alignment to original text.
    pub fn explain(
        &self,
        entry: usize,
        query: &QueryWorkspace,
        casing: Option<&[u32]>,
    ) -> Result<Option<Explanation>, SearchError> {
        self.validate_query(query)?;
        let entry = self
            .entries
            .get(entry)
            .ok_or(SearchError::InvalidInput("invalid intent reference"))?;
        let _query = self.reserve_working(query.capacity_bytes())?;
        let available = self.available_working_bytes()?;
        let _operation = self.reserve_working(available)?;
        let mut budget = Budget::new(0, available)?;

        let (classes, leading) = crate::search::classes::compile(query, casing, &mut budget)?;
        if !entry.possible(
            classes.len(),
            query.max_ambig(),
            leading || classes.iter().any(|class| class.looping),
        ) {
            return Ok(None);
        }

        let Some((text, level)) = matching::expansion(
            &entry.graph,
            &classes,
            leading,
            query.max_ambig(),
            entry.max_units,
            available - budget.used,
        )?
        else {
            return Ok(None);
        };

        budget.used = add(budget.used, mul(text.capacity(), 2)?)?;
        let mut normalized = budget.reserve(text.len())?;
        self.options
            .normalizer
            .normalize_into(&text, &mut normalized)
            .map_err(|_| SearchError::InvalidInput("intent expansion normalization"))?;

        let explanation = crate::explain::explain(
            &normalized,
            Some(&text),
            query,
            casing,
            available - budget.used,
        )?;
        if explanation.as_ref().is_none_or(|e| e.level != level) {
            return Err(SearchError::InvalidInput("intent alignment invariant"));
        }
        Ok(explanation)
    }
}
