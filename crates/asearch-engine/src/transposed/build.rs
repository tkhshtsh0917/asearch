//! Reserves and constructs corpus-bit masks under index capacity limits.

use super::{Block, Mask, TransposedEngine, TransposedOptions, eq::EqTable, symbols};
use crate::{
    Profile, ResourceLimits, SearchError,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

pub(super) fn build(
    texts: &[&[u16]],
    options: TransposedOptions,
    limits: ResourceLimits,
    normalized: bool,
) -> Result<TransposedEngine, SearchError> {
    limits.validate()?;
    options.validate()?;
    let width = mul(options.block_words, 64)?;
    if texts.len() > limits.max_texts {
        return Err(SearchError::ResourceLimit("corpus exceeds text limit"));
    }
    let mut raw_units = 0;
    let mut longest = 0;
    for text in texts {
        if text.len() > u16::MAX as usize {
            return Err(SearchError::ResourceLimit(
                "text exceeds 65535 UTF-16 units",
            ));
        }
        raw_units = add(raw_units, text.len())?;
        longest = longest.max(text.len());
    }
    // Bound input work even when the owner has already normalized the text.
    if mul(raw_units, 2)? > limits.max_total_bytes {
        return Err(SearchError::ResourceLimit(
            "corpus input exceeds byte limit",
        ));
    }
    let mut budget = Budget::new(size_of::<TransposedEngine>(), limits.max_working_bytes)?;
    let mut scratch = budget.reserve(if normalized { 0 } else { longest })?;
    let mut lengths = budget.reserve(texts.len())?;
    let mut units = 0;
    let mut entries_count = 0;
    for text in texts {
        let input = if normalized {
            *text
        } else {
            options
                .normalizer
                .normalize_into(text, &mut scratch)
                .map_err(|_| SearchError::ResourceLimit("normalization capacity"))?;
            &scratch
        };
        units = add(units, input.len())?;
        if units > limits.max_units {
            return Err(SearchError::ResourceLimit(
                "corpus exceeds normalized unit limit",
            ));
        }
        let count = if options.profile == Profile::AsearchTsLib {
            input.len()
        } else {
            symbols(input).count()
        };
        entries_count = add(entries_count, count)?;
        lengths.push(count);
    }

    let (words, block_count, max_block_words) = measure(&lengths, width)?;
    let slots = mul(add(words, 1)?, 64)?;
    if slots > limits.max_slots || slots > u32::MAX as usize {
        return Err(SearchError::ResourceLimit("corpus exceeds slot limit"));
    }
    let mut masks: Vec<Mask> = budget.reserve(add(words, 1)?)?;
    masks.resize(words + 1, Mask::default());
    let mut blocks = budget.reserve(block_count)?;
    let mut entries = budget.reserve(entries_count)?;

    let mut slot = 0;
    let mut block_end = 0;
    for (text, &count) in texts.iter().zip(&lengths) {
        if slot + count + 1 > block_end {
            slot = block_end;
            let words = options.block_words.max((count + 1).div_ceil(64));
            block_end = slot + words * 64;
            blocks.push(Block {
                start: slot / 64,
                words,
            });
        }
        masks[slot / 64].sep |= 1 << (slot % 64);
        let end = slot + count;
        masks[end / 64].end |= 1 << (end % 64);
        let input = if normalized {
            *text
        } else {
            options
                .normalizer
                .normalize_into(text, &mut scratch)
                .map_err(|_| SearchError::ResourceLimit("normalization capacity"))?;
            &scratch
        };
        if options.profile == Profile::AsearchTsLib {
            for &unit in input {
                slot += 1;
                record(&mut masks, &mut entries, slot, u32::from(unit));
            }
        } else {
            for symbol in symbols(input) {
                slot += 1;
                record(&mut masks, &mut entries, slot, symbol);
            }
        }
        slot += 1;
    }
    let mut before = 0;
    for mask in &mut masks {
        mask.end_before = before;
        before += mask.end.count_ones() as usize;
    }
    budget.release(scratch);
    budget.release(lengths);
    let eq = EqTable::build(&mut entries, &mut budget)?;
    budget.release(entries);
    Ok(TransposedEngine {
        options,
        limits,
        masks,
        blocks,
        eq,
        texts: texts.len(),
        max_block_words,
        capacity_bytes: budget.used,
    })
}

fn measure(lengths: &[usize], width: usize) -> Result<(usize, usize, usize), SearchError> {
    let mut slot = 0;
    let mut end = 0;
    let mut blocks = 0;
    let mut max_words = 0;
    for &length in lengths {
        let needed = add(length, 1)?;
        if add(slot, needed)? > end {
            slot = end;
            let words = (width / 64).max(needed.div_ceil(64));
            end = add(slot, mul(words, 64)?)?;
            max_words = max_words.max(words);
            blocks += 1;
        }
        slot = add(slot, needed)?;
    }
    Ok((end / 64, blocks, max_words))
}

fn record(masks: &mut [Mask], entries: &mut Vec<(u32, u32)>, slot: usize, symbol: u32) {
    masks[slot / 64].text |= 1 << (slot % 64);
    entries.push((symbol, slot as u32));
}
