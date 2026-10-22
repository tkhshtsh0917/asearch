//! Validates flat UTF-16 columns and borrows grouped document rows for serialization.

use asearch_index::{Doc, ResourceLimits, SearchError, TransposedOptions};
/// Borrows complete text slices from flat columns, preserving lone surrogates and accounting for staging capacity.
pub fn build_flat(
    units: &[u16],
    lengths: &[u32],
    keys: &[u32],
    orders: &[u32],
    tombstones: &[u32],
    options: TransposedOptions,
    limits: ResourceLimits,
) -> Result<Vec<u8>, SearchError> {
    let transport = units
        .len()
        .checked_mul(2)
        .and_then(|n| {
            lengths
                .len()
                .checked_add(keys.len())
                .and_then(|m| m.checked_add(orders.len()))
                .and_then(|m| m.checked_add(tombstones.len()))
                .and_then(|m| m.checked_mul(4))
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(SearchError::ResourceLimit("flat transport overflow"))?;

    let limits = ResourceLimits {
        max_working_bytes: limits
            .max_working_bytes
            .checked_sub(transport)
            .filter(|&n| n > 0)
            .ok_or(SearchError::ResourceLimit("flat transport budget"))?,
        ..limits
    };

    if lengths.len() != keys.len() || keys.len() != orders.len() {
        return Err(SearchError::InvalidInput(
            "flat document arrays differ in length",
        ));
    }

    if lengths.len() > limits.max_texts || units.len() > limits.max_total_bytes / 4 {
        return Err(SearchError::ResourceLimit("flat document input budget"));
    }

    let predicted = lengths
        .len()
        .checked_mul(std::mem::size_of::<&[u16]>() + std::mem::size_of::<Doc<'_>>())
        .ok_or(SearchError::ResourceLimit("flat input capacity overflow"))?;
    if predicted >= limits.max_working_bytes {
        return Err(SearchError::ResourceLimit("flat input capacity budget"));
    }

    let mut refs = Vec::new();
    refs.try_reserve_exact(lengths.len())
        .map_err(|_| SearchError::ResourceLimit("flat input reservation"))?;

    let predicted = refs
        .capacity()
        .checked_mul(std::mem::size_of::<&[u16]>())
        .and_then(|a| {
            lengths
                .len()
                .checked_mul(std::mem::size_of::<Doc<'_>>())
                .and_then(|b| a.checked_add(b))
        })
        .ok_or(SearchError::ResourceLimit("flat input capacity overflow"))?;
    if predicted >= limits.max_working_bytes {
        return Err(SearchError::ResourceLimit("flat input capacity budget"));
    }

    let mut offset = 0usize;
    for &length in lengths {
        let end = offset
            .checked_add(length as usize)
            .filter(|&end| end <= units.len())
            .ok_or(SearchError::InvalidInput("flat text length mismatch"))?;
        refs.push(&units[offset..end]);
        offset = end;
    }
    if offset != units.len() {
        return Err(SearchError::InvalidInput("flat input contains extra units"));
    }

    let mut docs = Vec::new();
    docs.try_reserve_exact(lengths.len())
        .map_err(|_| SearchError::ResourceLimit("flat input reservation"))?;
    let mut i = 0;
    while i < keys.len() {
        let start = i;
        i += 1;
        while i < keys.len() && keys[i] == keys[start] {
            if orders[i] != orders[start] {
                return Err(SearchError::InvalidInput(
                    "document order differs between texts",
                ));
            }
            i += 1;
        }
        docs.push(Doc {
            key: keys[start],
            order: orders[start],
            texts: &refs[start..i],
        });
    }

    let staging = refs
        .capacity()
        .checked_mul(std::mem::size_of::<&[u16]>())
        .and_then(|a| {
            docs.capacity()
                .checked_mul(std::mem::size_of::<Doc<'_>>())
                .and_then(|b| a.checked_add(b))
        })
        .ok_or(SearchError::ResourceLimit("flat input capacity overflow"))?;

    // Staging references remain alive during construction. Reserve their
    // capacity before asking the core to build its owning copy.
    let remaining = limits
        .max_working_bytes
        .checked_sub(staging)
        .filter(|&n| n > 0)
        .ok_or(SearchError::ResourceLimit("flat input capacity budget"))?;

    crate::build(
        &docs,
        tombstones,
        options,
        ResourceLimits {
            max_working_bytes: remaining,
            ..limits
        },
    )
}
