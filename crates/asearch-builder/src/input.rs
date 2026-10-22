//! Validates document identities and raw input sizes before normalization.

use asearch_index::{
    Doc, ResourceLimits, SearchError,
    capacity::{Budget, add, mul},
};

/// Reserves canonical references without copying the caller's text buffers.
pub(super) fn canonical_docs<'a, 't>(
    docs: &'a [Doc<'t>],
    budget: &mut Budget,
) -> Result<Vec<&'a Doc<'t>>, SearchError> {
    let mut sorted: Vec<&Doc<'t>> = budget.reserve(docs.len())?;
    sorted.extend(docs.iter());
    sorted.sort_unstable_by_key(|doc| doc.key);
    if sorted.windows(2).any(|p| p[0].key == p[1].key) {
        return Err(SearchError::InvalidInput("duplicate document key"));
    }
    sorted.sort_unstable_by_key(|doc| (doc.order, doc.key));
    Ok(sorted)
}

/// Counts raw text storage before the builder reserves its staging buffers.
pub(super) fn shape(
    docs: &[&Doc<'_>],
    limits: ResourceLimits,
) -> Result<(usize, usize), SearchError> {
    let mut count = 0;
    let mut units = 0;
    for doc in docs {
        if doc.texts.is_empty() {
            return Err(SearchError::InvalidInput("document requires texts"));
        }
        count = add(count, doc.texts.len())?;
        for text in doc.texts {
            if text.len() > u16::MAX as usize {
                return Err(SearchError::ResourceLimit("text length"));
            }
            units = add(units, text.len())?;
        }
    }
    if count > limits.max_texts || mul(units, 4)? > limits.max_total_bytes {
        return Err(SearchError::ResourceLimit("builder input budget"));
    }
    Ok((count, units))
}
