use super::{invalid, u16_at, view::View};
use crate::{
    AsearchIndex, Doc, ResourceLimits, SearchError, TransposedOptions,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

pub(in crate::index) fn load(
    bytes: &[u8],
    options: TransposedOptions,
    limits: ResourceLimits,
    reserved_input: usize,
) -> Result<AsearchIndex, SearchError> {
    limits.validate()?;
    if reserved_input > limits.max_total_bytes {
        return Err(SearchError::ResourceLimit("ASIX owned input budget"));
    }
    let view = View::parse(bytes, options, limits, reserved_input)?;
    let original = view.original.unwrap_or(view.normalized);
    let lengths = view.original_lengths.unwrap_or(view.lengths);
    let units = original.len() / 2;
    let staging_input = add(original.len(), reserved_input)?;
    if staging_input >= limits.max_total_bytes {
        return Err(SearchError::ResourceLimit("ASIX staging input budget"));
    }
    let mut budget = Budget::new(reserved_input, limits.max_working_bytes)?;
    let mut decoded: Vec<u16> = budget.reserve(units)?;
    let staging_input = add(mul(decoded.capacity(), 2)?, reserved_input)?;
    if staging_input >= limits.max_total_bytes {
        return Err(SearchError::ResourceLimit("ASIX staging input budget"));
    }
    decoded.extend(
        original
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]])),
    );
    let mut refs = budget.reserve(view.texts)?;
    let mut offset = 0;
    for i in 0..view.texts {
        let len = u16_at(lengths, i * 2) as usize;
        refs.push(&decoded[offset..offset + len]);
        offset += len;
    }
    let groups = (0..view.texts)
        .filter(|&i| i == 0 || view.key(i) != view.key(i - 1))
        .count();
    let mut docs = budget.reserve(groups)?;
    let mut i = 0;
    while i < view.texts {
        let start = i;
        let key = view.key(i);
        let order = view.order(i);
        i += 1;
        while i < view.texts && view.key(i) == key {
            i += 1;
        }
        docs.push(Doc {
            key,
            order,
            texts: &refs[start..i],
        });
    }
    if add(budget.used, size_of::<AsearchIndex>())? > limits.max_working_bytes {
        return Err(SearchError::ResourceLimit("ASIX construction budget"));
    }
    let tomb_data = view.tombstones.unwrap_or(&[]);
    let mut tombstones = budget.reserve(tomb_data.len() / 4)?;
    tombstones.extend(
        tomb_data
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte chunk"))),
    );
    let tomb_bytes = mul(tombstones.capacity(), 4)?;
    let staging_input = add(staging_input, tomb_bytes)?;
    if staging_input >= limits.max_total_bytes {
        return Err(SearchError::ResourceLimit("ASIX tombstone input budget"));
    }
    let mut index = AsearchIndex::from_docs(
        &docs,
        options,
        ResourceLimits {
            max_working_bytes: limits.max_working_bytes - budget.used,
            max_total_bytes: limits.max_total_bytes - staging_input,
            ..limits
        },
    )
    .map_err(|error| match error {
        SearchError::InvalidInput(_) => invalid(),
        other => other,
    })?;
    let mut offset = 0;
    for i in 0..view.texts {
        let len = u16_at(view.lengths, i * 2) as usize;
        let expected = view.normalized[offset..offset + len * 2]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
        if !index
            .text(i)
            .ok_or_else(invalid)?
            .iter()
            .copied()
            .eq(expected)
        {
            return Err(invalid());
        }
        offset += len * 2;
    }
    if view.original.is_none() {
        index.has_original = false;
        index.capacity_bytes -= mul(index.original.capacity(), 2)?;
        index.original = Vec::new();
    }
    index.tombstones = tombstones;
    index.capacity_bytes = add(index.capacity_bytes, tomb_bytes)?;
    index.source = Some(super::super::SourceInfo {
        id: u64::from_le_bytes(bytes[24..32].try_into().expect("validated header")),
        bytes: bytes.len(),
    });
    index.limits = limits;
    index.engine.set_working_budget(
        limits.max_working_bytes - (index.capacity_bytes - index.engine.capacity_bytes()),
    );
    Ok(index)
}
