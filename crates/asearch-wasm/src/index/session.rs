use super::{IndexHandle, error};
use asearch_rs::SearchError;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl IndexHandle {
    /// Independent query state sharing the immutable owned corpus.
    pub fn fork_session(
        &mut self,
        max_cache_bytes: u32,
        max_ambig: u32,
    ) -> Result<IndexHandle, JsError> {
        let max_ambig = u8::try_from(max_ambig)
            .ok()
            .filter(|&k| k <= 7)
            .ok_or_else(|| error(SearchError::InvalidInput("session K must be in 0..7")))?;
        if self.workspace.is_none() {
            self.workspace = Some(self.index.workspace(&self.query, 7).map_err(error)?);
        }
        let mut session = Self::shared(self.index.clone(), self.options, self.limits)?;
        let mut workspace = session
            .index
            .workspace(&session.query, max_ambig)
            .map_err(error)?;
        // Keep room for the largest packed result even when the cache would
        // otherwise consume all remaining instance capacity.
        let packed = session
            .index
            .document_count()
            .checked_mul(16)
            .and_then(|n| n.checked_add(56))
            .ok_or_else(|| {
                error(SearchError::ResourceLimit(
                    "session result capacity overflow",
                ))
            })?;
        let cache_budget = session
            .index
            .available_working_bytes()
            .map_err(error)?
            .saturating_sub(packed.max(session.query.pattern_capacity().saturating_mul(2)));
        workspace
            .enable_incremental(
                &session.index,
                &session.query,
                (max_cache_bytes as usize).min(cache_budget),
            )
            .map_err(error)?;
        session.workspace = Some(workspace);
        Ok(session)
    }

    pub fn reset_session(&mut self) -> Result<(), JsError> {
        self.query.update(&[]).map_err(error)?;
        if let Some(workspace) = &mut self.workspace {
            workspace.reset_incremental();
        }
        Ok(())
    }
}
