//! Compiles bounded intent expressions into normalized, symbol-aware DAGs.

use super::{Entry, IntentEntry, IntentIndex, graph::Quota, parser, transform};
use crate::{
    Profile, ResourceLimits, SearchError, TransposedOptions,
    capacity::{Budget, add, mul},
    index::reservations::OVERHEAD,
};
use std::{mem::size_of, sync::Arc};

impl IntentIndex {
    /// Parses entries without enumerating expansions and charges every graph allocation.
    pub fn from_entries(
        input: &[IntentEntry<'_>],
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        options.validate()?;
        limits.validate()?;
        if input.len() > limits.max_texts {
            return Err(SearchError::ResourceLimit("intent entry count"));
        }

        let units = input
            .iter()
            .try_fold(0usize, |n, entry| add(n, entry.expr.len()))?;
        if units > limits.max_units || mul(units, 2)? > limits.max_total_bytes {
            return Err(SearchError::ResourceLimit("intent input units"));
        }

        let staging = add(
            mul(units, 2)?,
            mul(input.len(), size_of::<IntentEntry<'_>>())?,
        )?;

        let mut budget = Budget::new(
            add(add(size_of::<Self>(), OVERHEAD)?, staging)?,
            limits.max_working_bytes,
        )?;

        let mut entries: Vec<Entry> = budget.reserve(input.len())?;
        let mut raw_quota = Quota::raw(limits.max_slots);
        let mut compiled_quota = Quota::compiled(limits.max_slots);

        // Reject duplicate identities before building any graph.
        let mut keys = budget.reserve(input.len())?;
        keys.extend(input.iter().map(|entry| entry.key));
        keys.sort_unstable();
        if keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SearchError::InvalidInput("duplicate intent key"));
        }
        budget.release(keys);

        for entry in input {
            let mut graph = parser::parse(entry.expr, &mut budget, &mut raw_quota)?;
            if options.normalizer != crate::Normalizer::NONE {
                graph = transform::compile(
                    graph,
                    transform::Kind::Normalize(options.normalizer),
                    &mut budget,
                    &mut compiled_quota,
                )?;
            }
            if options.profile != Profile::AsearchTsLib {
                graph = transform::compile(
                    graph,
                    transform::Kind::Symbols,
                    &mut budget,
                    &mut compiled_quota,
                )?;
            }
            let (min_symbols, max_symbols) = graph.length_bounds(&mut budget)?;
            entries.push(Entry {
                key: entry.key,
                order: entry.order,
                graph,
                max_units: entry.expr.len(),
                min_symbols,
                max_symbols,
            });
        }

        entries.sort_unstable_by_key(|entry| (entry.order, entry.key));

        Ok(Self {
            entries,
            options,
            limits,
            capacity_bytes: budget.used - staging,
            reservations: Arc::default(),
        })
    }
}
