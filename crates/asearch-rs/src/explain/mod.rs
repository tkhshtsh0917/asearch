//! Bounded top-result alignment. This path intentionally allocates, unlike
//! prepared bulk search, and rejects matrices exceeding the remaining budget.
mod input;
mod trace;
use crate::{
    QueryWorkspace, SearchError,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateSpace {
    OriginalUtf16,
    NormalizedUtf16,
}
#[derive(Debug)]
pub struct Explanation {
    pub text: Vec<u16>,
    pub level: u16,
    pub spans: Vec<(usize, usize)>,
    pub coordinate_space: CoordinateSpace,
}
impl Explanation {
    pub fn capacity_bytes(&self) -> usize {
        size_of::<Self>()
            + self.text.capacity() * 2
            + self.spans.capacity() * size_of::<(usize, usize)>()
    }
}
#[derive(Clone, Copy)]
pub(super) struct Class {
    keys: [u32; 3],
    looping: bool,
}
#[derive(Clone, Copy)]
pub(super) struct Symbol {
    value: u32,
    start: usize,
    end: usize,
}

pub(crate) fn explain(
    normalized: &[u16],
    original: Option<&[u16]>,
    query: &QueryWorkspace,
    casing: Option<&[u32]>,
    available: usize,
) -> Result<Option<Explanation>, SearchError> {
    let mut budget = Budget::new(size_of::<Explanation>(), available)?;
    let input::Input {
        text,
        mapping,
        symbols,
    } = input::text(normalized, original, query, casing, &mut budget)?;
    let (classes, leading) = input::pattern(query, casing, &mut budget)?;
    let cells = mul(add(classes.len(), 1)?, add(symbols.len(), 1)?)?;
    let mut matrix = budget.reserve(cells)?;
    matrix.resize(cells, trace::Cell::default());
    let mut selected = budget.reserve(symbols.len())?;
    let level = trace::align(&classes, &symbols, leading, &mut matrix, &mut selected);
    if level > u16::from(query.max_ambig()) {
        return Ok(None);
    }
    let mut spans: Vec<(usize, usize)> = budget.reserve(selected.len())?;
    for symbol in selected.iter().rev() {
        let (start, _) = mapping[symbol.start];
        let (_, end) = mapping[symbol.end - 1];
        if let Some((_, previous_end)) = spans.last_mut()
            && *previous_end >= start
        {
            *previous_end = (*previous_end).max(end);
        } else {
            spans.push((start, end));
        }
    }
    Ok(Some(Explanation {
        text,
        level,
        spans,
        coordinate_space: if original.is_some() {
            CoordinateSpace::OriginalUtf16
        } else {
            CoordinateSpace::NormalizedUtf16
        },
    }))
}
