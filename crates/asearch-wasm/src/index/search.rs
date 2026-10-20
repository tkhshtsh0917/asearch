use super::{IndexHandle, config, error};
use asearch_rs::SearchError;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl IndexHandle {
    /// Normalization and ambiguity selection happen once before JS compiles
    /// the host-locale casing candidates from this wrapped UTF-16 pattern.
    pub fn prepare_u16(&mut self, units: &[u16], config: &[u32]) -> Result<Vec<u16>, JsError> {
        let options = config::query_config(self.options, config).map_err(error)?;
        self.query
            .update_with_options(units, options)
            .map_err(error)?;
        let mut pattern = Vec::new();
        let count = self.query.pattern().len();
        let live = self.live_bytes()?;
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
            .and_then(|k| asearch_rs::AmbigPolicy::fixed(k).ok())
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
        if self.workspace.is_none() {
            self.workspace = Some(self.index.workspace(&self.query, 7).map_err(error)?);
        }
        let live = self.live_bytes()?;
        let workspace = self.workspace.as_mut().expect("workspace prepared above");
        let hits = (limit as usize).min(self.index.document_count());
        let count = hits
            .checked_mul(4)
            .and_then(|v| v.checked_add(14))
            .ok_or_else(|| error(SearchError::ResourceLimit("result capacity overflow")))?;
        let bytes = count
            .checked_mul(4)
            .and_then(|v| v.checked_add(live))
            .ok_or_else(|| error(SearchError::ResourceLimit("result capacity overflow")))?;
        if bytes > self.limits.max_working_bytes {
            return Err(error(SearchError::ResourceLimit("result capacity budget")));
        }
        let mut packed = Vec::new();
        packed
            .try_reserve_exact(count)
            .map_err(|_| error(SearchError::ResourceLimit("result reservation")))?;
        if packed.capacity() > count
            && bytes
                .checked_add((packed.capacity() - count) * 4)
                .is_none_or(|n| n > self.limits.max_working_bytes)
        {
            return Err(error(SearchError::ResourceLimit("result capacity budget")));
        }
        let result = self
            .index
            .search_with_casing(&self.query, keys, workspace, limit as usize, count_total)
            .map_err(error)?;
        packed.push(result.hits.len() as u32);
        packed.push(result.totals.map_or(0, |totals| totals.len()) as u32);
        packed.extend_from_slice(&[0, 0, 0, 0]);
        for hit in result.hits {
            packed.extend_from_slice(&[hit.key, hit.order, hit.text_id as u32, hit.level as u32]);
        }
        if let Some(totals) = result.totals {
            packed.extend(totals.iter().map(|&n| n as u32));
        }
        let stats = workspace.stats();
        packed[2] = stats.blocks as u32;
        packed[3] = stats.early_exit_blocks as u32;
        packed[4] = stats.narrowed_texts.unwrap_or(stats.reused_rows) as u32;
        packed[5] = if stats.narrowed_texts.is_some() {
            2
        } else {
            u32::from(stats.incremental)
        };
        Ok(packed)
    }
}
