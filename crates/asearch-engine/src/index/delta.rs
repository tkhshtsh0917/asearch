//! Constructs a native segment whose tombstones invalidate older documents only.

use super::{AsearchIndex, Doc};
use crate::{
    ResourceLimits, SearchError, TransposedOptions,
    capacity::{Budget, mul},
};

pub(super) fn build(
    docs: &[Doc<'_>],
    tombstones: &[u32],
    options: TransposedOptions,
    limits: ResourceLimits,
) -> Result<AsearchIndex, SearchError> {
    limits.validate()?;
    options.validate()?;
    let mut budget = Budget::new(0, limits.max_working_bytes.min(limits.max_total_bytes))?;
    let mut keys = budget.reserve(tombstones.len())?;
    keys.extend_from_slice(tombstones);
    keys.sort_unstable();
    if keys.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SearchError::InvalidInput("duplicate tombstone key"));
    }
    let bytes = mul(keys.capacity(), 4)?;
    if bytes >= limits.max_total_bytes || bytes >= limits.max_working_bytes {
        return Err(SearchError::ResourceLimit("tombstone capacity budget"));
    }
    let mut index = AsearchIndex::from_docs(
        docs,
        options,
        ResourceLimits {
            max_total_bytes: limits.max_total_bytes - bytes,
            max_working_bytes: limits.max_working_bytes - bytes,
            ..limits
        },
    )?;
    index.capacity_bytes += bytes;
    index.tombstones = keys;
    index.limits = limits;
    index.engine.set_working_budget(
        limits.max_working_bytes - (index.capacity_bytes - index.engine.capacity_bytes()),
    );
    Ok(index)
}
