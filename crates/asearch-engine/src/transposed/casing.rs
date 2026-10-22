//! Validates host-provided case candidates before compiling symbol classes.

use super::symbols;
use crate::{QueryWorkspace, SearchError};

/// Three keys per wrapped pattern code point: original, lower, upper.
/// u32::MAX denotes a multi-code-point expansion that cannot match one slot.
pub(crate) fn validate(query: &QueryWorkspace, keys: &[u32]) -> Result<(), SearchError> {
    let expected = symbols(query.pattern())
        .count()
        .checked_mul(3)
        .ok_or(SearchError::ResourceLimit("casing key count overflow"))?;
    if keys.len() != expected || keys.iter().any(|&key| key > 0x10ffff && key != u32::MAX) {
        return Err(SearchError::InvalidInput("invalid casing key triples"));
    }
    if symbols(query.pattern())
        .zip(keys.chunks_exact(3))
        .any(|(symbol, keys)| symbol != keys[0])
    {
        return Err(SearchError::InvalidInput(
            "casing original differs from pattern",
        ));
    }
    Ok(())
}
