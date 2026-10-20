//! Bounded acyclic intent expressions. Public APIs search server-built entries;
//! no mutable editing layer, regex runtime or expansion enumeration is used.
mod graph;
mod matching;
mod parser;
mod transform;
use crate::{
    Explanation, Profile, QueryWorkspace, ResourceLimits, SearchError, TransposedOptions,
    WorkingReservation,
    capacity::{Budget, add, mul},
    index::reservations::{Lease, OVERHEAD, Reservations},
};
use graph::{Graph, Quota};
use std::{mem::size_of, sync::Arc};

#[derive(Clone, Copy, Debug)]
pub struct IntentEntry<'a> {
    pub key: u32,
    pub order: u32,
    pub expr: &'a [u16],
}
#[derive(Debug)]
struct Entry {
    key: u32,
    order: u32,
    graph: Graph,
    max_units: usize,
    min_symbols: usize,
    max_symbols: usize,
}
impl Entry {
    fn possible(&self, literals: usize, k: u8, free_loop: bool) -> bool {
        literals <= self.max_symbols.saturating_add(usize::from(k))
            && (free_loop || self.min_symbols <= literals.saturating_add(usize::from(k)))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentHit {
    pub key: u32,
    pub order: u32,
    pub entry: usize,
    pub level: u16,
}
#[derive(Debug)]
pub struct IntentResults {
    pub hits: Vec<IntentHit>,
    pub totals: Option<[usize; 8]>,
}
impl IntentResults {
    pub fn capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.hits.capacity() * size_of::<IntentHit>()
    }
}
#[derive(Debug)]
pub struct IntentIndex {
    entries: Vec<Entry>,
    options: TransposedOptions,
    limits: ResourceLimits,
    capacity_bytes: usize,
    reservations: Arc<Reservations>,
}
impl IntentIndex {
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
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
    pub fn graph_size(&self) -> (usize, usize) {
        self.entries.iter().fold((0, 0), |(nodes, edges), entry| {
            (nodes + entry.graph.nodes(), edges + entry.graph.edges.len())
        })
    }
    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }
    pub fn available_working_bytes(&self) -> Result<usize, SearchError> {
        let used = self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("intent accounting unavailable"))?;
        Ok(self
            .limits
            .max_working_bytes
            .saturating_sub(add(self.capacity_bytes, *used)?))
    }
    pub fn reserve_working(&self, bytes: usize) -> Result<WorkingReservation, SearchError> {
        let mut used = self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("intent accounting unavailable"))?;
        if add(add(self.capacity_bytes, *used)?, bytes)? > self.limits.max_working_bytes {
            return Err(SearchError::ResourceLimit(
                "intent external working reservation",
            ));
        }
        *used = add(*used, bytes)?;
        Ok(WorkingReservation(Lease::new(
            self.reservations.clone(),
            bytes,
        )))
    }
    fn validate_query(&self, query: &QueryWorkspace) -> Result<(), SearchError> {
        if query.options().normalizer != self.options.normalizer
            || query.options().profile != self.options.profile
        {
            return Err(SearchError::InvalidInput(
                "query and intent descriptors differ",
            ));
        }
        if query.input_units() > self.limits.max_pattern_units
            || query.normalized().len() > self.limits.max_pattern_units
        {
            return Err(SearchError::ResourceLimit("intent pattern limit"));
        }
        Ok(())
    }
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
