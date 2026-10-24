//! Prepares shared query buffers and per-segment search reservations.

use super::IndexSet;
use crate::{
    IndexWorkspace, QueryWorkspace, SearchError, SearchHit,
    capacity::{Budget, add},
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentHit {
    pub segment: usize,
    pub hit: SearchHit,
}

#[derive(Debug)]
pub struct SetResults<'a> {
    pub hits: &'a [SegmentHit],
    pub totals: Option<&'a [usize]>,
}

#[derive(Debug)]
pub struct SetWorkspace {
    parts: Vec<IndexWorkspace>,
    output: Vec<SegmentHit>,
    totals: [usize; 8],
    cursors: [usize; 8],
    lease: super::super::reservations::Lease,
    // Drop last, after child and metadata reservations have been released.
    _activity: Activity,
}

#[derive(Debug)]
struct Activity(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl Drop for Activity {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(super) fn prepare(
    set: &IndexSet,
    query: &QueryWorkspace,
    max_ambig: u8,
) -> Result<SetWorkspace, SearchError> {
    let mut own = set
        .reservations
        .0
        .lock()
        .map_err(|_| SearchError::ResourceLimit("set accounting unavailable"))?;
    let base = add(set.base_live_bytes()?, *own)?;
    let mut budget = Budget::new(
        add(base, size_of::<SetWorkspace>())?,
        set.limits.max_working_bytes,
    )?;
    let mut parts = budget.reserve(set.segments.len())?;
    let output = budget.reserve(set.document_count())?;
    let metadata = budget.used - base;
    for segment in &set.segments {
        let remaining = set.limits.max_working_bytes - budget.used;
        let child_live = segment.limits.max_working_bytes - segment.available_working_bytes()?;
        let part = super::super::workspace::prepare_limited(
            segment,
            query,
            max_ambig,
            add(child_live, remaining)?,
        )?;
        budget.used = add(
            budget.used,
            add(part.capacity_bytes(), query.capacity_bytes())?,
        )?;
        parts.push(part);
    }
    *own = add(*own, metadata)?;
    set.activity
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(SetWorkspace {
        parts,
        output,
        totals: [0; 8],
        cursors: [0; 8],
        lease: super::super::reservations::Lease::new(set.reservations.clone(), metadata),
        _activity: Activity(set.activity.clone()),
    })
}

pub(super) fn search<'w>(
    set: &IndexSet,
    query: &QueryWorkspace,
    keys: Option<&[u32]>,
    ws: &'w mut SetWorkspace,
    limit: usize,
    count_total: bool,
) -> Result<SetResults<'w>, SearchError> {
    if !ws.lease.owns(&set.reservations) || ws.parts.len() != set.segments.len() {
        return Err(SearchError::InvalidInput(
            "set workspace belongs to another index",
        ));
    }
    if query.input_units() > set.limits.max_pattern_units
        || query.normalized().len() > set.limits.max_pattern_units
    {
        return Err(SearchError::ResourceLimit(
            "query exceeds set pattern limit",
        ));
    }
    let mut totals = [0usize; 8];
    for (i, (segment, part)) in set.segments.iter().zip(&mut ws.parts).enumerate() {
        let live =
            (set.segments.len() > 1).then_some(&set.masks[set.offsets[i]..set.offsets[i + 1]]);
        let result = segment.search_compiled(query, keys, part, limit, count_total, live)?;
        if let Some(part_totals) = result.totals {
            for (total, &count) in totals.iter_mut().zip(part_totals) {
                *total = add(*total, count)?;
            }
        }
    }
    // Adoption proves live keys are disjoint across segments. Exact minimum-
    // level counts can be summed, unlike arbitrary overlapping Worker shards.
    ws.output.clear();
    ws.totals = totals;
    ws.cursors.fill(0);
    while ws.output.len() < limit {
        let selected = ws
            .parts
            .iter()
            .enumerate()
            .filter_map(|(i, part)| part.hits().get(ws.cursors[i]).map(|hit| (i, *hit)))
            .min_by_key(|(i, hit)| (hit.level, hit.start, hit.order, hit.key, *i));
        let Some((segment, hit)) = selected else {
            break;
        };
        ws.cursors[segment] += 1;
        ws.output.push(SegmentHit { segment, hit });
    }
    Ok(SetResults {
        hits: &ws.output,
        totals: count_total.then_some(&ws.totals[..=query.max_ambig() as usize]),
    })
}

impl SetWorkspace {
    /// Shares the cache allowance across segments and respects reservations
    /// from every other session. A segment may safely keep the dense path.
    pub fn enable_planner(
        &mut self,
        set: &IndexSet,
        query: &QueryWorkspace,
        max_cache_bytes: usize,
        result_reserve: usize,
    ) -> Result<(), SearchError> {
        if !self.lease.owns(&set.reservations) {
            return Err(SearchError::InvalidInput(
                "set workspace belongs to another index",
            ));
        }
        if max_cache_bytes == 0 {
            return Ok(());
        }
        // Serialize capacity-changing setup across the entire column. Child
        // ledgers alone cannot prevent two segments from spending the same
        // root remainder concurrently.
        let own = set
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("set accounting unavailable"))?;
        let available = || -> Result<usize, SearchError> {
            Ok(set
                .limits
                .max_working_bytes
                .saturating_sub(add(set.base_live_bytes()?, *own)?)
                .saturating_sub(result_reserve))
        };
        for (segment, part) in set.segments.iter().zip(&mut self.parts) {
            let available = available()?;
            match part.enable_narrowing(segment, query, available) {
                Ok(()) | Err(SearchError::ResourceLimit(_)) => (),
                Err(error) => return Err(error),
            }
        }
        let mut remaining = max_cache_bytes;
        for (segment, part) in set.segments.iter().zip(&mut self.parts) {
            let available = available()?;
            let before = part.capacity_bytes();
            part.enable_incremental(segment, query, remaining.min(available))?;
            remaining = remaining.saturating_sub(part.capacity_bytes() - before);
        }
        Ok(())
    }

    pub fn reset_incremental(&mut self) {
        for part in &mut self.parts {
            part.reset_incremental();
        }
    }

    pub fn stats(&self) -> impl Iterator<Item = crate::EngineStats> + '_ {
        self.parts.iter().map(IndexWorkspace::stats)
    }
}
