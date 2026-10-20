use super::{IndexHandle, error};
use asearch_rs::{CoordinateSpace, SearchError};
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
impl IndexHandle {
    pub fn explain_prepared(
        &self,
        reference: u32,
        keys: &[u32],
        original: &[u16],
        supplied: bool,
    ) -> Result<Vec<u32>, JsError> {
        let result = (|| -> Result<Vec<u32>, SearchError> {
            let shared = self.index.borrow();
            let flat = shared.refs.resolve(reference)?;
            let (segment, text) = shared
                .index
                .text_location(flat)
                .ok_or(SearchError::InvalidInput("invalid text reference"))?;
            let Some(explanation) = shared.index.explain(
                segment,
                text,
                &self.query,
                Some(keys),
                supplied.then_some(original),
            )?
            else {
                return Ok(Vec::new());
            };
            let _result = shared.index.reserve_working(explanation.capacity_bytes())?;
            let count = explanation
                .spans
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(explanation.text.len()))
                .and_then(|n| n.checked_add(4))
                .ok_or(SearchError::ResourceLimit("explanation result overflow"))?;
            let bytes = count
                .checked_mul(4)
                .ok_or(SearchError::ResourceLimit("explanation result overflow"))?;
            let _packed = shared.index.reserve_working(bytes)?;
            let mut packed = Vec::new();
            packed
                .try_reserve_exact(count)
                .map_err(|_| SearchError::ResourceLimit("explanation result reservation"))?;
            if packed.capacity() != count {
                return Err(SearchError::ResourceLimit("explanation result capacity"));
            }
            packed.extend_from_slice(&[
                u32::from(explanation.level),
                u32::from(explanation.coordinate_space == CoordinateSpace::OriginalUtf16),
                explanation.text.len() as u32,
                explanation.spans.len() as u32,
            ]);
            packed.extend(explanation.text.iter().map(|&unit| u32::from(unit)));
            for (start, end) in explanation.spans {
                packed.extend_from_slice(&[start as u32, end as u32]);
            }
            Ok(packed)
        })();
        result.map_err(error)
    }
}
