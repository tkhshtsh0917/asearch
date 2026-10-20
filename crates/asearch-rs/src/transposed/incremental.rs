//! Bounded checkpoints of complete corpus rows. Terminal rows are never
//! checkpoints: their accept loop must move when another literal is typed.
use super::{
    TransposedEngine, matching,
    workspace::{Class, TransposedWorkspace},
};
use crate::{
    QueryWorkspace, SearchError,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

#[derive(Debug)]
pub(crate) struct IncrementalCache {
    previous: Vec<u64>,
    current: Vec<u64>,
    rows: Vec<u64>,
    prefixes: Vec<usize>,
    history: Vec<Class>,
    leading: bool,
    valid: bool,
    next: usize,
    stride: usize,
    levels: usize,
    bytes: usize,
}

impl IncrementalCache {
    /// A cache that cannot hold even one checkpoint safely falls back to the
    /// blocked engine. All capacities, including scratch rows, fit max_bytes.
    pub(crate) fn new(
        engine: &TransposedEngine,
        query: &QueryWorkspace,
        max_ambig: u8,
        max_bytes: usize,
    ) -> Result<Option<Self>, SearchError> {
        let stride = engine.masks.len();
        let levels = usize::from(max_ambig) + 1;
        let row = mul(stride, levels)?;
        let row_bytes = mul(row, size_of::<u64>())?;
        let metadata = add(
            size_of::<Self>(),
            mul(query.pattern_capacity(), size_of::<Class>())?,
        )?;
        let minimum = add(metadata, add(mul(row_bytes, 3)?, size_of::<usize>())?)?;
        if minimum > max_bytes {
            return Ok(None);
        }
        let entries = ((max_bytes - metadata - row_bytes * 2) / (row_bytes + size_of::<usize>()))
            .min(query.pattern_capacity());
        let mut budget = Budget::new(size_of::<Self>(), max_bytes)?;
        let history = budget.reserve(query.pattern_capacity())?;
        let mut previous = budget.reserve(row)?;
        previous.resize(row, 0);
        let mut current = budget.reserve(row)?;
        current.resize(row, 0);
        let mut rows = budget.reserve(mul(entries, row)?)?;
        rows.resize(entries * row, 0);
        let mut prefixes = budget.reserve(entries)?;
        prefixes.resize(entries, usize::MAX);
        Ok(Some(Self {
            previous,
            current,
            rows,
            prefixes,
            history,
            leading: false,
            valid: false,
            next: 0,
            stride,
            levels,
            bytes: budget.used,
        }))
    }

    pub(crate) fn capacity_bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn reset(&mut self) {
        self.valid = false;
        self.prefixes.fill(usize::MAX);
        self.history.clear();
        self.next = 0;
    }

    pub(crate) fn search(
        &mut self,
        engine: &TransposedEngine,
        query: &QueryWorkspace,
        ws: &mut TransposedWorkspace,
        casing: Option<&[u32]>,
    ) {
        let leading = matching::compile(engine, query, &mut ws.classes, casing);
        let common = if self.valid && self.leading == leading {
            self.history
                .iter()
                .zip(&ws.classes)
                .take_while(|(a, b)| a == b)
                .count()
                .min(self.history.len().saturating_sub(1))
                .min(ws.classes.len().saturating_sub(1))
        } else {
            0
        };
        for prefix in &mut self.prefixes {
            if *prefix > common {
                *prefix = usize::MAX;
            }
        }
        let cached = self
            .prefixes
            .iter()
            .enumerate()
            .filter(|(_, p)| **p != usize::MAX && **p <= common)
            .max_by_key(|(_, p)| **p)
            .map(|(slot, &prefix)| (slot, prefix));
        self.previous.fill(0);
        self.current.fill(0);
        let row_size = self.previous.len();
        let resume = if let Some((slot, prefix)) = cached {
            self.previous
                .copy_from_slice(&self.rows[slot * row_size..(slot + 1) * row_size]);
            prefix
        } else {
            for &block in &engine.blocks {
                matching::row(
                    engine,
                    block,
                    &self.current[block.start..],
                    &mut self.previous[block.start..],
                    &[],
                    self.stride,
                    self.levels,
                    0,
                    leading,
                );
            }
            0
        };
        for (i, class) in ws.classes.iter().enumerate().skip(resume) {
            for &block in &engine.blocks {
                let eq = &mut ws.eq_cache[..block.words];
                eq.fill(0);
                let mut previous_id = 0;
                for id in class.ids {
                    if id != previous_id {
                        engine.eq.scatter(id, block.start, eq);
                    }
                    previous_id = id;
                }
                matching::row(
                    engine,
                    block,
                    &self.previous[block.start..],
                    &mut self.current[block.start..],
                    eq,
                    self.stride,
                    self.levels,
                    i + 1,
                    class.loop_after,
                );
            }
            std::mem::swap(&mut self.previous, &mut self.current);
            // An explicit wildcard loop inside a prefix is persistent. The
            // terminal accept loop is recomputed and never reused as a prefix.
            if i + 1 < ws.classes.len() {
                let slot = self.next;
                self.rows[slot * row_size..(slot + 1) * row_size].copy_from_slice(&self.previous);
                self.prefixes[slot] = i + 1;
                self.next = (self.next + 1) % self.prefixes.len();
            }
        }
        ws.levels.fill(None);
        for &block in &engine.blocks {
            matching::extract(
                engine,
                block,
                &self.previous[block.start..],
                self.stride,
                usize::from(query.max_ambig()) + 1,
                &mut ws.levels,
            );
        }
        self.history.clear();
        self.history.extend_from_slice(&ws.classes);
        self.leading = leading;
        self.valid = true;
        ws.stats.blocks = engine.blocks.len();
        ws.stats.early_exit_blocks = 0;
        ws.stats.narrowed_texts = None;
        ws.stats.reused_rows = resume;
    }
}
