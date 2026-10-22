//! Validates result references and packs original-coordinate explanations for JS.

use super::{IndexHandle, error};
use asearch_engine::{CoordinateSpace, SearchError};
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
            let bytes = keys
                .len()
                .checked_mul(4)
                .and_then(|n| original.len().checked_mul(2).and_then(|m| n.checked_add(m)))
                .ok_or(SearchError::ResourceLimit(
                    "explanation input capacity overflow",
                ))?;
            let _input = shared.index.reserve_working(bytes)?;
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
            pack(explanation, |bytes| shared.index.reserve_working(bytes))
        })();
        result.map_err(error)
    }
}

pub(crate) fn pack(
    explanation: asearch_engine::Explanation,
    reserve: impl Fn(usize) -> Result<asearch_engine::WorkingReservation, SearchError>,
) -> Result<Vec<u32>, SearchError> {
    let _result = reserve(explanation.capacity_bytes())?;
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
    let _packed = reserve(bytes)?;
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
}
