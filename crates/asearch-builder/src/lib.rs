//! Server-side canonical ASIX v1 construction and serialization.
#![forbid(unsafe_code)]
mod encoding;
use asearch_index::{
    ResourceLimits, SearchError, TransposedOptions,
    capacity::{Budget, add, mul},
};
pub use encoding::{Encoding, Row, encode, encoded_size};
/// Builds canonical server bytes without constructing a search engine.
pub fn build(
    docs: &[asearch_index::Doc<'_>],
    tombstones: &[u32],
    options: TransposedOptions,
    limits: ResourceLimits,
) -> Result<Vec<u8>, SearchError> {
    limits.validate()?;
    options.validate()?;
    let mut budget = Budget::new(0, limits.max_working_bytes)?;
    let mut sorted: Vec<&asearch_index::Doc<'_>> = budget.reserve(docs.len())?;
    sorted.extend(docs.iter());
    sorted.sort_unstable_by_key(|doc| doc.key);
    if sorted.windows(2).any(|p| p[0].key == p[1].key) {
        return Err(SearchError::InvalidInput("duplicate document key"));
    }
    sorted.sort_unstable_by_key(|doc| (doc.order, doc.key));
    let mut count = 0;
    let mut units = 0;
    for doc in &sorted {
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
    let mut normalized = budget.reserve(units)?;
    let mut ranges: Vec<(usize, usize, &[u16], u32, u32)> = budget.reserve(count)?;
    let mut scratch = budget.reserve(
        docs.iter()
            .flat_map(|d| d.texts.iter())
            .map(|t| t.len())
            .max()
            .unwrap_or(0),
    )?;
    for doc in &sorted {
        for original in doc.texts {
            options
                .normalizer
                .normalize_into(original, &mut scratch)
                .map_err(|_| SearchError::ResourceLimit("normalization capacity"))?;
            let start = normalized.len();
            normalized.extend_from_slice(&scratch);
            ranges.push((start, normalized.len(), *original, doc.key, doc.order));
        }
    }
    if normalized.len() > limits.max_units {
        return Err(SearchError::ResourceLimit("normalized unit budget"));
    }
    budget.release(scratch);
    let mut tombs = budget.reserve(tombstones.len())?;
    tombs.extend_from_slice(tombstones);
    tombs.sort_unstable();
    if tombs.windows(2).any(|p| p[0] == p[1]) {
        return Err(SearchError::InvalidInput("duplicate tombstone"));
    }
    encode(
        ranges
            .iter()
            .map(|&(start, end, original, key, order)| Row {
                normalized: &normalized[start..end],
                original: Some(original),
                key,
                order,
            }),
        &tombs,
        Encoding {
            options,
            has_original: true,
            limits,
            live_bytes: budget.used,
        },
    )
}

mod flat;
pub use flat::build_flat;
