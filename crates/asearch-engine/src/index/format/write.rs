//! Serializes native index rows while preserving originals and working reservations.

use super::encoding::{Encoding, Row, encode, encoded_size};
use crate::{AsearchIndex, SearchError};

fn settings(index: &AsearchIndex) -> Result<Encoding, SearchError> {
    Ok(Encoding {
        options: index.engine.options(),
        has_original: index.has_original,
        limits: index.limits,
        live_bytes: index.limits.max_working_bytes - index.available_working_bytes()?,
    })
}

fn rows(index: &AsearchIndex) -> impl Clone + Iterator<Item = Row<'_>> {
    index.documents.iter().flat_map(move |doc| {
        (doc.start..doc.end).map(move |i| Row {
            normalized: index.text(i).expect("owned text range"),
            original: index.original_text(i),
            key: doc.key,
            order: doc.order,
        })
    })
}
pub(in crate::index) fn size(index: &AsearchIndex) -> Result<usize, SearchError> {
    encoded_size(rows(index), &index.tombstones, settings(index)?)
}
pub(in crate::index) fn write(index: &AsearchIndex) -> Result<Vec<u8>, SearchError> {
    write_limited(index, index.limits.max_working_bytes)
}

pub(in crate::index) fn write_limited(
    index: &AsearchIndex,
    max_working_bytes: usize,
) -> Result<Vec<u8>, SearchError> {
    // Serialize transient allocation against other reservations on this same
    // index. The returned Vec becomes caller-owned after this guard is released.
    let live = index
        .reservations
        .0
        .lock()
        .map_err(|_| SearchError::ResourceLimit("workspace accounting unavailable"))?;
    let settings = Encoding {
        options: index.engine.options(),
        has_original: index.has_original,
        limits: crate::ResourceLimits {
            max_working_bytes: index.limits.max_working_bytes.min(max_working_bytes),
            ..index.limits
        },
        live_bytes: crate::capacity::add(index.capacity_bytes, *live)?,
    };
    let result = encode(rows(index), &index.tombstones, settings);
    drop(live);
    result
}
