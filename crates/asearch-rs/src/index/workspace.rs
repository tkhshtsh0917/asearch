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
    let mut live = index
        .reservations
        .0
        .lock()
        .map_err(|_| SearchError::ResourceLimit("workspace accounting unavailable"))?;
    let engine =
        crate::transposed::workspace::prepare_with_live(&index.engine, query, max_ambig, *live)?;
    let initial = add(
        add(add(index.capacity_bytes, query.capacity_bytes())?, *live)?,
        add(
            engine.capacity_bytes(),
            size_of::<IndexWorkspace>() - size_of::<TransposedWorkspace>(),
        )?,
    )?;
    let mut budget = Budget::new(initial, index.limits.max_working_bytes)?;
    let mut best = budget.reserve(index.documents.len())?;
    best.resize(index.documents.len(), None);
    let hits = budget.reserve(index.documents.len())?;
    let capacity_bytes = budget.used - index.capacity_bytes - query.capacity_bytes() - *live;
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
        lease: super::reservations::Lease::new(index.reservations.clone(), leased),
    })
}

impl IndexWorkspace {
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
) -> SearchResults<'w> {
    ws.hits.clear();
    ws.totals.fill(0);
    for (doc, best) in documents.iter().zip(&mut ws.best) {
        *best = None;
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
