mod config;
mod flat;

use asearch_rs::{
    AsearchIndex, IndexWorkspace, QueryOptions, QueryWorkspace, ResourceLimits, SearchError,
    TransposedOptions,
};
use wasm_bindgen::prelude::*;

fn error(error: SearchError) -> JsError {
    JsError::new(&format!("{}: {error}", error.code()))
}

#[wasm_bindgen]
pub struct IndexHandle {
    index: AsearchIndex,
    query: QueryWorkspace,
    workspace: Option<IndexWorkspace>,
    options: TransposedOptions,
    limits: ResourceLimits,
}

impl IndexHandle {
    fn wrap(
        index: AsearchIndex,
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, JsError> {
        let remaining = limits
            .max_working_bytes
            .checked_sub(index.capacity_bytes())
            .filter(|&v| v > 0)
            .ok_or_else(|| error(SearchError::ResourceLimit("query capacity budget")))?;
        let query = QueryWorkspace::new(
            QueryOptions {
                normalizer: options.normalizer,
                profile: options.profile,
                ..QueryOptions::default()
            },
            ResourceLimits {
                max_working_bytes: remaining,
                ..limits
            },
        )
        .map_err(error)?;
        Ok(Self {
            index,
            query,
            workspace: None,
            options,
            limits,
        })
    }
}

#[wasm_bindgen]
impl IndexHandle {
    pub fn from_flat(
        units: &[u16],
        lengths: &[u32],
        keys: &[u32],
        orders: &[u32],
        config: &[u32],
    ) -> Result<IndexHandle, JsError> {
        let (options, limits) = config::index_config(config).map_err(error)?;
        let index = flat::build(units, lengths, keys, orders, options, limits).map_err(error)?;
        Self::wrap(index, options, limits)
    }

    pub fn from_bytes(bytes: &[u8], config: &[u32]) -> Result<IndexHandle, JsError> {
        let (options, limits) = config::index_config(config).map_err(error)?;
        Self::wrap(
            AsearchIndex::from_bytes(bytes, options, limits).map_err(error)?,
            options,
            limits,
        )
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, JsError> {
        let serialized_size = self.index.serialized_size().map_err(error)?;
        let required = self
            .index
            .capacity_bytes()
            .checked_add(self.query.capacity_bytes())
            .and_then(|v| {
                v.checked_add(
                    self.workspace
                        .as_ref()
                        .map_or(0, IndexWorkspace::capacity_bytes),
                )
            })
            .and_then(|v| v.checked_add(serialized_size))
            .ok_or_else(|| {
                error(SearchError::ResourceLimit(
                    "serialization capacity overflow",
                ))
            })?;
        if required > self.limits.max_working_bytes {
            return Err(error(SearchError::ResourceLimit(
                "serialization capacity budget",
            )));
        }
        self.index.to_bytes().map_err(error)
    }

    /// Packed copied results: hit count, totals count, four u32s per hit,
    /// followed by exact totals. No JS objects enter the Rust search loop.
    pub fn search_u16(
        &mut self,
        units: &[u16],
        config: &[u32],
        limit: u32,
        count_total: bool,
    ) -> Result<Vec<u32>, JsError> {
        let options = config::query_config(self.options, config).map_err(error)?;
        self.query
            .update_with_options(units, options)
            .map_err(error)?;
        if self.workspace.is_none() {
            self.workspace = Some(self.index.workspace(&self.query, 7).map_err(error)?);
        }
        let workspace = self.workspace.as_mut().expect("workspace prepared above");
        let hits = (limit as usize).min(self.index.document_count());
        let count = hits
            .checked_mul(4)
            .and_then(|v| v.checked_add(12))
            .ok_or_else(|| error(SearchError::ResourceLimit("result capacity overflow")))?;
        let bytes = count
            .checked_mul(4)
            .and_then(|v| v.checked_add(self.index.capacity_bytes()))
            .and_then(|v| v.checked_add(self.query.capacity_bytes()))
            .and_then(|v| v.checked_add(workspace.capacity_bytes()))
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
            .search(&self.query, workspace, limit as usize, count_total)
            .map_err(error)?;
        packed.push(result.hits.len() as u32);
        packed.push(result.totals.map_or(0, |totals| totals.len()) as u32);
        packed.extend_from_slice(&[0, 0]);
        for hit in result.hits {
            packed.extend_from_slice(&[hit.key, hit.order, hit.text_id as u32, hit.level as u32]);
        }
        if let Some(totals) = result.totals {
            packed.extend(totals.iter().map(|&n| n as u32));
        }
        let stats = workspace.stats();
        packed[2] = stats.blocks as u32;
        packed[3] = stats.early_exit_blocks as u32;
        Ok(packed)
    }
}
