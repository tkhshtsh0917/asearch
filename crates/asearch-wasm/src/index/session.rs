use super::{IndexHandle, error};
use asearch_engine::SearchError;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl IndexHandle {
    pub fn fork_session(
        &mut self,
        max_cache_bytes: u32,
        max_ambig: u32,
    ) -> Result<IndexHandle, JsError> {
        let result = (|| -> Result<Self, SearchError> {
            let max_ambig = u8::try_from(max_ambig)
                .ok()
                .filter(|&k| k <= 7)
                .ok_or(SearchError::InvalidInput("session K must be in 0..7"))?;
            self.ensure_workspace()?;
            let mut session = Self::shared(self.index.clone(), self.options, self.limits)?;
            session.cache = Some((max_cache_bytes as usize, max_ambig));
            session.ensure_workspace()?;
            Ok(session)
        })();
        result.map_err(error)
    }
    pub fn reset_session(&mut self) -> Result<(), JsError> {
        self.query.update(&[]).map_err(error)?;
        if let Some(workspace) = &mut self.workspace {
            workspace.reset_incremental();
        }
        Ok(())
    }
    pub fn clear_workspace(&mut self) {
        self.workspace = None;
    }
}
