//! Bounded acyclic intent expressions. Public APIs search server-built entries;
//! no mutable editing layer, regex runtime or expansion enumeration is used.

mod build;
mod execution;
mod graph;
mod matching;
mod parser;
mod transform;
use crate::{
    QueryWorkspace, ResourceLimits, SearchError, TransposedOptions, WorkingReservation,
    capacity::add,
    index::reservations::{Lease, Reservations},
};
use graph::Graph;
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
    /// Returns owned capacity, excluding separately leased operation buffers.
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
    /// Returns the number of stable document identities in this intent index.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the aggregate compiled node and edge counts without enumerating expansions.
    pub fn graph_size(&self) -> (usize, usize) {
        self.entries.iter().fold((0, 0), |(nodes, edges), entry| {
            (nodes + entry.graph.nodes(), edges + entry.graph.edges.len())
        })
    }

    /// Returns owned capacity, excluding separately leased operation buffers.
    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    /// Reports the remaining shared working budget after active reservations.
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

    /// Leases external working capacity until the returned guard is dropped.
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
}
