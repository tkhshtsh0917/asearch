//! Owns reusable row buffers and validates their engine/query compatibility.

use super::{EngineStats, TransposedEngine};
use crate::capacity::{Budget, add, mul};
use crate::{QueryWorkspace, SearchError};
use std::mem::size_of;

pub(super) const CACHE_ROWS: usize = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Class {
    pub ids: [u32; 3],
    pub loop_after: bool,
}

/// Prepared outside search. It may be reused with queries no longer than the
/// reserved pattern and engines whose block/output requirements fit.
#[derive(Debug)]
pub struct TransposedWorkspace {
    pub(super) previous: Vec<u64>,
    pub(super) current: Vec<u64>,
    pub(super) eq_cache: Vec<u64>,
    pub(super) classes: Vec<Class>,
    pub(super) levels: Vec<Option<u16>>,
    pub(super) max_ambig: u8,
    pub(super) words: usize,
    pattern_units: usize,
    capacity_bytes: usize,
    pub(super) stats: EngineStats,
}

pub(super) fn prepare(
    engine: &TransposedEngine,
    query: &QueryWorkspace,
    max_ambig: u8,
) -> Result<TransposedWorkspace, SearchError> {
    prepare_with_live(engine, query, max_ambig, 0)
}

pub(crate) fn prepare_with_live(
    engine: &TransposedEngine,
    query: &QueryWorkspace,
    max_ambig: u8,
    live: usize,
) -> Result<TransposedWorkspace, SearchError> {
    check_query(engine, query)?;
    if max_ambig > 7 || max_ambig < query.max_ambig() {
        return Err(SearchError::InvalidInput(
            "workspace requires query K <= max_ambig <= 7",
        ));
    }
    let initial = add(
        add(add(engine.capacity_bytes, query.capacity_bytes())?, live)?,
        size_of::<TransposedWorkspace>(),
    )?;
    let mut budget = Budget::new(initial, engine.limits.max_working_bytes)?;
    let words = engine.max_block_words;
    let states = mul(words, max_ambig as usize + 1)?;
    let mut previous = budget.reserve(states)?;
    previous.resize(states, 0);
    let mut current = budget.reserve(states)?;
    current.resize(states, 0);
    let mut eq_cache = budget.reserve(mul(words, CACHE_ROWS)?)?;
    eq_cache.resize(words * CACHE_ROWS, 0);
    let classes = budget.reserve(query.pattern_capacity())?;
    let mut levels = budget.reserve(engine.texts)?;
    levels.resize(engine.texts, None);
    let workspace = TransposedWorkspace {
        previous,
        current,
        eq_cache,
        classes,
        levels,
        max_ambig,
        words,
        pattern_units: query.pattern_capacity(),
        capacity_bytes: budget.used - engine.capacity_bytes - query.capacity_bytes() - live,
        stats: EngineStats::default(),
    };
    workspace.validate(engine, query)?;
    Ok(workspace)
}

fn check_query(engine: &TransposedEngine, query: &QueryWorkspace) -> Result<(), SearchError> {
    let options = query.options();
    if options.normalizer != engine.options.normalizer || options.profile != engine.options.profile
    {
        return Err(SearchError::InvalidInput(
            "query and corpus descriptors differ",
        ));
    }
    if query.input_units() > engine.limits.max_pattern_units
        || query.normalized().len() > engine.limits.max_pattern_units
    {
        return Err(SearchError::ResourceLimit(
            "query exceeds engine pattern limit",
        ));
    }
    Ok(())
}

impl TransposedWorkspace {
    pub(crate) fn classes(&self) -> &[Class] {
        &self.classes
    }

    pub(crate) fn classes_mut(&mut self) -> &mut Vec<Class> {
        &mut self.classes
    }

    pub(crate) fn narrowing_parts(&mut self) -> (&[Class], &mut [Option<u16>]) {
        (&self.classes, &mut self.levels)
    }

    pub(crate) fn set_narrowing_stats(&mut self, texts: usize) {
        self.stats = EngineStats {
            narrowed_texts: Some(texts),
            ..EngineStats::default()
        };
    }

    pub(crate) fn levels(&self) -> &[Option<u16>] {
        &self.levels
    }

    pub(crate) fn max_ambig(&self) -> u8 {
        self.max_ambig
    }

    pub fn stats(&self) -> EngineStats {
        self.stats
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    pub(crate) fn validate(
        &self,
        engine: &TransposedEngine,
        query: &QueryWorkspace,
    ) -> Result<(), SearchError> {
        check_query(engine, query)?;
        if query.max_ambig() > self.max_ambig
            || query.pattern().len() > self.pattern_units
            || engine.max_block_words > self.words
            || engine.texts != self.levels.len()
        {
            return Err(SearchError::ResourceLimit(
                "transposed workspace is too small",
            ));
        }
        let total = add(
            add(engine.capacity_bytes, query.capacity_bytes())?,
            self.capacity_bytes,
        )?;
        if total > engine.limits.max_working_bytes {
            return Err(SearchError::ResourceLimit("transposed capacity budget"));
        }
        Ok(())
    }
}
