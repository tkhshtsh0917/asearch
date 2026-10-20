mod builder;
mod config;
mod expected;
mod flat;
mod search;
mod session;
mod stream;

use asearch_rs::{
    AsearchIndex, IndexWorkspace, QueryOptions, QueryWorkspace, ResourceLimits, SearchError,
    TransposedOptions,
};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

pub(super) fn error(error: SearchError) -> JsError {
    JsError::new(&format!("{}: {error}", error.code()))
}

#[wasm_bindgen]
pub struct IndexHandle {
    index: Arc<AsearchIndex>,
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
        Self::shared(Arc::new(index), options, limits)
    }

    fn live_bytes(&self) -> Result<usize, JsError> {
        let used =
            self.limits.max_working_bytes - self.index.available_working_bytes().map_err(error)?;
        used.checked_add(if self.workspace.is_none() {
            self.query.capacity_bytes()
        } else {
            0
        })
        .ok_or_else(|| error(SearchError::ResourceLimit("live capacity overflow")))
    }

    fn shared(
        index: Arc<AsearchIndex>,
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, JsError> {
        let remaining = index.available_working_bytes().map_err(error)?;
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
        let index =
            flat::build(units, lengths, keys, orders, &[], options, limits).map_err(error)?;
        Self::wrap(index, options, limits)
    }

    pub fn from_bytes(
        bytes: Vec<u8>,
        config: &[u32],
        expected: &[u8],
    ) -> Result<IndexHandle, JsError> {
        let (options, limits) = config::index_config(config).map_err(error)?;
        Self::wrap(
            expected::load(bytes, options, limits, expected).map_err(error)?,
            options,
            limits,
        )
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, JsError> {
        let serialized_size = self.index.serialized_size().map_err(error)?;
        let required = self
            .live_bytes()?
            .checked_add(serialized_size)
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
}
