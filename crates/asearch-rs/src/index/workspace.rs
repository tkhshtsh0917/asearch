use super::{AsearchIndex, Document};
use crate::{
    QueryWorkspace, SearchError, TransposedWorkspace,
    capacity::{Budget, add},
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchHit {
    pub key: u32,
    pub order: u32,
    pub text_id: usize,
    pub level: u16,
}

#[derive(Debug)]
pub struct SearchResults<'a> {
    pub hits: &'a [SearchHit],
    /// Exact minimum-level counts of distinct document keys, independent of
    /// the requested result limit. Absent when counting was not requested.
    pub totals: Option<&'a [usize]>,
}

#[derive(Debug)]
pub struct IndexWorkspace {
    pub(super) engine: TransposedWorkspace,
    pub(super) output: OutputBuffers,
    capacity_bytes: usize,
    lease: super::reservations::Lease,
    pub(super) narrowing: Option<super::narrowing::Narrowing>,
    pub(super) incremental: Option<crate::transposed::incremental::IncrementalCache>,
}

#[derive(Debug)]
pub(super) struct OutputBuffers {
    best: Vec<Option<SearchHit>>,
    hits: Vec<SearchHit>,
    totals: [usize; 8],
}

pub(super) fn prepare(
    index: &AsearchIndex,
    query: &QueryWorkspace,
    max_ambig: u8,
) -> Result<IndexWorkspace, SearchError> {
    prepare_limited(index, query, max_ambig, usize::MAX)
}

pub(super) fn prepare_limited(
    index: &AsearchIndex,
    query: &QueryWorkspace,
    max_ambig: u8,
    max_working_bytes: usize,
) -> Result<IndexWorkspace, SearchError> {
    let padding = index
        .limits
        .max_working_bytes
        .saturating_sub(max_working_bytes);
    let mut live = index
        .reservations
        .0
        .lock()
        .map_err(|_| SearchError::ResourceLimit("workspace accounting unavailable"))?;
    let engine = crate::transposed::workspace::prepare_with_live(
        &index.engine,
        query,
        max_ambig,
        add(*live, padding)?,
    )?;
    let initial = add(
        add(
            add(add(index.capacity_bytes, query.capacity_bytes())?, *live)?,
            padding,
        )?,
        add(
            engine.capacity_bytes(),
            size_of::<IndexWorkspace>() - size_of::<TransposedWorkspace>(),
        )?,
    )?;
    let mut budget = Budget::new(initial, index.limits.max_working_bytes)?;
    let mut best = budget.reserve(index.documents.len())?;
    best.resize(index.documents.len(), None);
    let hits = budget.reserve(index.documents.len())?;
    let capacity_bytes =
        budget.used - index.capacity_bytes - query.capacity_bytes() - *live - padding;
    let leased = add(capacity_bytes, query.capacity_bytes())?;
    *live = add(*live, leased)?;
    Ok(IndexWorkspace {
        engine,
        output: OutputBuffers {
            best,
            hits,
            totals: [0; 8],
        },
        capacity_bytes,
        incremental: None,
        narrowing: None,
        lease: super::reservations::Lease::new(index.reservations.clone(), leased),
    })
}

impl IndexWorkspace {
    /// Enables bounded row checkpoints. Insufficient cache space is a safe
    /// blocked-search fallback, not a failure of an otherwise valid search.
    pub fn enable_incremental(
        &mut self,
        index: &AsearchIndex,
        query: &QueryWorkspace,
        max_bytes: usize,
    ) -> Result<bool, SearchError> {
        self.validate(index, query)?;
        if self.incremental.is_some() {
            return Err(SearchError::InvalidInput(
                "incremental cache already configured",
            ));
        }
        let mut live = index
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("workspace accounting unavailable"))?;
        let available = index
            .limits
            .max_working_bytes
            .saturating_sub(add(index.capacity_bytes, *live)?);
        let cache = crate::transposed::incremental::IncrementalCache::new(
            &index.engine,
            query,
            self.engine.max_ambig(),
            max_bytes.min(available),
        )?;
        if let Some(cache) = cache {
            let bytes = cache.capacity_bytes();
            *live = add(*live, bytes)?;
            self.lease.grow(bytes);
            self.capacity_bytes = add(self.capacity_bytes, bytes)?;
            self.incremental = Some(cache);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Reserves complete candidates and scalar DP buffers for safe narrowing.
    pub fn enable_narrowing(
        &mut self,
        index: &AsearchIndex,
        query: &QueryWorkspace,
        max_bytes: usize,
    ) -> Result<(), SearchError> {
        self.validate(index, query)?;
        if self.narrowing.is_some() {
            return Err(SearchError::InvalidInput("narrowing already configured"));
        }
        let mut live = index
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("workspace accounting unavailable"))?;
        let available = index
            .limits
            .max_working_bytes
            .saturating_sub(add(index.capacity_bytes, *live)?);
        let planner = super::narrowing::Narrowing::new(index, query, available.min(max_bytes))?;
        let bytes = planner.capacity_bytes();
        *live = add(*live, bytes)?;
        self.lease.grow(bytes);
        self.capacity_bytes = add(self.capacity_bytes, bytes)?;
        self.narrowing = Some(planner);
        Ok(())
    }

    pub fn is_incremental(&self) -> bool {
        self.incremental.is_some()
    }

    pub fn reset_incremental(&mut self) {
        if let Some(planner) = &mut self.narrowing {
            planner.reset();
        }
        if let Some(cache) = &mut self.incremental {
            cache.reset();
        }
    }

    pub(super) fn hits(&self) -> &[SearchHit] {
        &self.output.hits
    }

    pub fn stats(&self) -> crate::EngineStats {
        self.engine.stats()
    }
    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    pub(super) fn validate(
        &self,
        index: &AsearchIndex,
        query: &QueryWorkspace,
    ) -> Result<(), SearchError> {
        if !self.lease.owns(&index.reservations) {
            return Err(SearchError::InvalidInput(
                "workspace belongs to another index",
            ));
        }
        self.lease
            .validate_query(query.capacity_bytes(), self.capacity_bytes)?;
        let total = add(
            add(index.capacity_bytes, query.capacity_bytes())?,
            self.capacity_bytes,
        )?;
        if index.documents.len() != self.output.best.len() || total > index.limits.max_working_bytes
        {
            return Err(SearchError::ResourceLimit(
                "index workspace capacity budget",
            ));
        }
        Ok(())
    }
}

// Called only after all fallible validation and corpus DP have succeeded.
pub(super) fn extract<'w>(
    documents: &[Document],
    levels: &[Option<u16>],
    k: u8,
    ws: &'w mut OutputBuffers,
    limit: usize,
    count_total: bool,
    live: Option<&[u8]>,
) -> SearchResults<'w> {
    ws.hits.clear();
    ws.totals.fill(0);
    for (i, (doc, best)) in documents.iter().zip(&mut ws.best).enumerate() {
        *best = None;
        if live.is_some_and(|mask| mask[i] == 0) {
            continue;
        }
        for (offset, &level) in levels[doc.start..doc.end].iter().enumerate() {
            if let Some(level) = level
                && best.is_none_or(|hit| level < hit.level)
            {
                *best = Some(SearchHit {
                    key: doc.key,
                    order: doc.order,
                    text_id: doc.start + offset,
                    level,
                });
            }
        }
        if count_total && let Some(hit) = best {
            ws.totals[hit.level as usize] += 1;
        }
    }
    // Documents already have canonical (order,key) order. Level buckets avoid
    // a query-time sort; equal-cost texts keep their original text order.
    'levels: for level in 0..=k {
        for &hit in ws.best.iter().flatten() {
            if ws.hits.len() >= limit {
                break 'levels;
            }
            if hit.level == u16::from(level) {
                ws.hits.push(hit);
            }
        }
    }
    SearchResults {
        hits: &ws.hits,
        totals: count_total.then_some(&ws.totals[..=k as usize]),
    }
}
