//! Server-side canonical ASIX v1 construction and serialization.

#![forbid(unsafe_code)]
mod encoding;
mod flat;
mod input;

use asearch_index::{ResourceLimits, SearchError, TransposedOptions, capacity::Budget};

pub use encoding::{Encoding, Row, encode, encoded_size};
pub use flat::build_flat;

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
    let sorted = input::canonical_docs(docs, &mut budget)?;
    let (count, units) = input::shape(&sorted, limits)?;

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
