use super::{IndexHandle, config, error, flat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl IndexHandle {
    /// Server-side immutable delta builder, not an edit of an active index.
    pub fn build_delta_flat(
        units: &[u16],
        lengths: &[u32],
        keys: &[u32],
        orders: &[u32],
        tombstones: &[u32],
        config: &[u32],
    ) -> Result<Vec<u8>, JsError> {
        let (options, limits) = config::index_config(config).map_err(error)?;
        let index = flat::build(units, lengths, keys, orders, tombstones, options, limits)
            .map_err(error)?;
        index.to_bytes().map_err(error)
    }
}
