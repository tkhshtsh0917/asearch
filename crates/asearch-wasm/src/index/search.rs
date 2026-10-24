//! Prepares host-cased queries and packs search results with bounded transport buffers.

use super::{IndexHandle, config, error};
use asearch_engine::SearchError;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl IndexHandle {
    /// Normalization and ambiguity selection happen once before JS compiles
    /// the host-locale casing candidates from this wrapped UTF-16 pattern.
    pub fn prepare_u16(&mut self, units: &[u16], config: &[u32]) -> Result<Vec<u16>, JsError> {
        let options = config::query_config(self.options, config).map_err(error)?;
        let input_bytes = units
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(config.len() * 4))
            .ok_or_else(|| error(SearchError::ResourceLimit("query input capacity overflow")))?;
        let reservation = { self.index.borrow().index.reserve_working(input_bytes) };
        let _input = reservation.map_err(error)?;
        self.query
            .update_with_options(units, options)
            .map_err(error)?;
        let mut pattern = Vec::new();
        let count = self.query.pattern().len();
        let live = self.live_bytes().map_err(error)?;
        let fits = |capacity: usize| {
            capacity
                .checked_mul(2)
                .and_then(|v| v.checked_add(live))
                .is_some_and(|v| v <= self.limits.max_working_bytes)
        };
        if !fits(count) {
            return Err(error(SearchError::ResourceLimit(
                "query copy capacity budget",
            )));
        }
        pattern
            .try_reserve_exact(count)
            .map_err(|_| error(SearchError::ResourceLimit("query copy reservation")))?;
        if !fits(pattern.capacity()) {
            return Err(error(SearchError::ResourceLimit(
                "query copy capacity budget",
            )));
        }
        pattern.extend_from_slice(self.query.pattern());
        Ok(pattern)
    }

    pub fn select_level(&mut self, level: u32) -> Result<(), JsError> {
        let level = u8::try_from(level)
            .ok()
            .and_then(|k| asearch_engine::AmbigPolicy::fixed(k).ok())
            .ok_or_else(|| {
                error(SearchError::InvalidInput(
                    "custom policy requires an integer in 0..7",
                ))
            })?;
        self.query.set_policy(level);
        Ok(())
    }

    /// Packed copied results: hit count, totals count, block stats, four u32s
    /// per hit, then exact totals. No JS objects enter the Rust search loop.
    pub fn search_prepared(
        &mut self,
        keys: &[u32],
        limit: u32,
        count_total: bool,
    ) -> Result<Vec<u32>, JsError> {
        let result = (|| -> Result<Vec<u32>, SearchError> {
            let input_bytes = keys
                .len()
                .checked_mul(4)
                .ok_or(SearchError::ResourceLimit("casing input capacity overflow"))?;
            let _input = self.index.borrow().index.reserve_working(input_bytes)?;
            self.ensure_workspace()?;
            let live = self.live_bytes()?;
            let workspace = self.workspace.as_mut().expect("workspace prepared");
            let mut shared = self.index.borrow_mut();
            let super::Shared { index, refs, .. } = &mut *shared;
            let hits = (limit as usize).min(index.document_count());
            let positional = self.query.options().rank_by_position;
            let count = packed_words(hits, positional)?;
            let bytes = count
                .checked_mul(4)
                .and_then(|n| n.checked_add(live))
                .ok_or(SearchError::ResourceLimit("result capacity overflow"))?;
            if bytes > self.limits.max_working_bytes {
                return Err(SearchError::ResourceLimit("result capacity budget"));
            }
            let mut packed = Vec::new();
            packed
                .try_reserve_exact(count)
                .map_err(|_| SearchError::ResourceLimit("result reservation"))?;
            if packed
                .capacity()
                .checked_mul(4)
                .and_then(|n| n.checked_add(live))
                .is_none_or(|n| n > self.limits.max_working_bytes)
            {
                return Err(SearchError::ResourceLimit("result capacity budget"));
            }
            let result = index.search_with_casing(
                &self.query,
                keys,
                workspace,
                limit as usize,
                count_total,
            )?;
            packed.extend_from_slice(&[
                result.hits.len() as u32,
                result.totals.map_or(0, |v| v.len()) as u32,
                0,
                0,
                0,
                0,
                index.segment_count() as u32,
                0,
            ]);
            for entry in result.hits {
                let hit = entry.hit;
                let flat = index
                    .text_id(entry.segment, hit.text_id)
                    .expect("validated hit");
                packed.extend_from_slice(&[hit.key, hit.order, refs.get(flat)?, hit.level as u32]);
            }
            if let Some(totals) = result.totals {
                packed.extend(totals.iter().map(|&n| n as u32));
            }
            if positional {
                packed.extend(
                    result
                        .hits
                        .iter()
                        .map(|entry| u32::try_from(entry.hit.start).unwrap_or(u32::MAX)),
                );
            }
            let mut engine = None;
            for stats in workspace.stats() {
                packed[2] += stats.blocks as u32;
                packed[3] += stats.early_exit_blocks as u32;
                packed[4] += stats.reused_rows as u32;
                let code = if let Some(count) = stats.narrowed_texts {
                    packed[7] += count as u32;
                    2
                } else {
                    u32::from(stats.incremental)
                };
                engine = Some(engine.map_or(code, |old| if old == code { old } else { 3 }));
            }
            packed[5] = engine.unwrap_or(0);
            Ok(packed)
        })();
        result.map_err(error)
    }
}

/// Includes the fixed header, maximum totals and the optional start column.
pub(super) fn packed_words(hits: usize, positional: bool) -> Result<usize, SearchError> {
    hits.checked_mul(if positional { 5 } else { 4 })
        .and_then(|words| words.checked_add(16))
        .ok_or(SearchError::ResourceLimit("result capacity overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planner_reservation_covers_both_result_layouts_without_overflow() {
        assert_eq!(packed_words(1000, false).unwrap(), 4016);
        assert_eq!(packed_words(1000, true).unwrap(), 5016);
        assert_eq!(packed_words(0, true).unwrap(), 16);
        assert!(packed_words(usize::MAX, true).is_err());
    }
}
