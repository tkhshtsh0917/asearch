//! Prepares bounded intent queries and translates results across the WASM boundary.

use super::*;
#[wasm_bindgen]
impl IntentHandle {
    pub fn prepare_u16(&mut self, units: &[u16], config: &[u32]) -> Result<Vec<u16>, JsError> {
        let result = (|| -> Result<Vec<u16>, SearchError> {
            let _input = self.index.reserve_working(
                units
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(config.len() * 4))
                    .ok_or(SearchError::ResourceLimit("intent query input capacity"))?,
            )?;
            let options = config::query_config(self.options, config)?;
            if options.rank_by_position {
                return Err(asearch_engine::SearchError::InvalidInput(
                    "positional ranking is only available for immutable indexes",
                ));
            }
            self.query.update_with_options(units, options)?;
            let count = self.query.pattern().len();
            let _copy = self.index.reserve_working(
                count
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(config.len() * 4))
                    .ok_or(SearchError::ResourceLimit("intent query copy capacity"))?,
            )?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(count)
                .map_err(|_| SearchError::ResourceLimit("intent query copy reservation"))?;
            if output.capacity() != count {
                return Err(SearchError::ResourceLimit("intent query copy capacity"));
            }
            output.extend_from_slice(self.query.pattern());
            Ok(output)
        })();
        result.map_err(error)
    }

    pub fn select_level(&mut self, level: u32) -> Result<(), JsError> {
        let result = u8::try_from(level)
            .ok()
            .and_then(|k| asearch_engine::AmbigPolicy::fixed(k).ok())
            .ok_or(SearchError::InvalidInput(
                "custom policy requires an integer in 0..7",
            ));
        self.query.set_policy(result.map_err(error)?);
        Ok(())
    }
}
