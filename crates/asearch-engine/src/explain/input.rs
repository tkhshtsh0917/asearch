use super::Symbol;
use crate::{Profile, QueryWorkspace, SearchError, capacity::Budget};

pub(super) struct Input {
    pub text: Vec<u16>,
    pub mapping: Vec<(usize, usize)>,
    pub symbols: Vec<Symbol>,
}
pub(super) fn text(
    normalized: &[u16],
    original: Option<&[u16]>,
    query: &QueryWorkspace,
    casing: Option<&[u32]>,
    budget: &mut Budget,
) -> Result<Input, SearchError> {
    if let Some(keys) = casing {
        crate::transposed::casing::validate(query, keys)?;
    }
    let display = original.unwrap_or(normalized);
    if display.len() > u16::MAX as usize {
        return Err(SearchError::ResourceLimit("explanation text unit limit"));
    }
    let mut text = budget.reserve(display.len())?;
    text.extend_from_slice(display);
    let mut transformed = budget.reserve(display.len())?;
    let mut mapping = budget.reserve(display.len())?;
    query
        .options()
        .normalizer
        .normalize_mapped_into(display, &mut transformed, &mut mapping)
        .map_err(|_| SearchError::InvalidInput("invalid original text"))?;
    if original.is_some() && transformed != normalized {
        return Err(SearchError::InvalidInput(
            "original does not normalize to indexed text",
        ));
    }
    // A normalized-only file already contains normalized units. Its positions
    // are reported explicitly in normalized coordinates, never as originals.
    if original.is_none() {
        mapping.clear();
        mapping.extend((0..normalized.len()).map(|i| (i, i + 1)));
    }
    budget.release(transformed);
    let mut symbols = budget.reserve(normalized.len())?;
    let mut offset = 0;
    if query.options().profile == Profile::AsearchTsLib {
        for (i, &unit) in normalized.iter().enumerate() {
            symbols.push(Symbol {
                value: u32::from(unit),
                start: i,
                end: i + 1,
            });
        }
    } else {
        for value in std::char::decode_utf16(normalized.iter().copied()) {
            let (value, width) = value.map_or_else(
                |e| (u32::from(e.unpaired_surrogate()), 1),
                |c| (u32::from(c), c.len_utf16()),
            );
            symbols.push(Symbol {
                value,
                start: offset,
                end: offset + width,
            });
            offset += width;
        }
    }
    Ok(Input {
        text,
        mapping,
        symbols,
    })
}
