use super::{ScalarError, ScalarLimits, ScalarWm, resources::checked_bytes};
use crate::{BitWord, MatchMode};
use std::mem::size_of;

impl<W: BitWord> ScalarWm<W> {
    /// Compiles the pattern, checking capacity and budget before reservation.
    pub fn new(pattern: &str, mode: MatchMode, limits: ScalarLimits) -> Result<Self, ScalarError> {
        limits.validate()?;
        let pattern_units = pattern.encode_utf16().count();
        if pattern_units > limits.max_pattern_units {
            return Err(ScalarError::PatternLimit);
        }
        let length = pattern
            .chars()
            .filter(|&c| mode != MatchMode::WildcardSpace || c != ' ')
            .count();
        if length >= W::BITS {
            return Err(ScalarError::WordCapacity);
        }

        let entries = length.checked_mul(3).ok_or(ScalarError::WorkingLimit)?;
        let base = size_of::<Self>();
        checked_bytes::<(u32, usize)>(entries, base, limits.max_working_bytes)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(entries)
            .map_err(|_| ScalarError::Reservation)?;
        let staging_bytes =
            checked_bytes::<(u32, usize)>(positions.capacity(), base, limits.max_working_bytes)?;

        let accept = W::bit(length);
        let mut loops = if mode == MatchMode::Include {
            W::bit(0).union(accept)
        } else {
            W::zero()
        };
        let mut position = 0;
        for c in pattern.chars() {
            if mode == MatchMode::WildcardSpace && c == ' ' {
                loops = loops.union(W::bit(position));
                continue;
            }
            positions.push((u32::from(c), position));
            for variant in [one_scalar(c.to_lowercase()), one_scalar(c.to_uppercase())]
                .into_iter()
                .flatten()
            {
                // A single scalar variant occupies the same pattern position.
                positions.push((u32::from(variant), position));
            }
            position += 1;
        }
        positions.sort_unstable_by_key(|&(symbol, _)| symbol);
        let mut previous = None;
        let mut unique = 0;
        for &(symbol, _) in &positions {
            if previous != Some(symbol) {
                unique += 1;
                previous = Some(symbol);
            }
        }
        checked_bytes::<(u32, W)>(unique, staging_bytes, limits.max_working_bytes)?;
        let mut masks: Vec<(u32, W)> = Vec::new();
        masks
            .try_reserve_exact(unique)
            .map_err(|_| ScalarError::Reservation)?;
        checked_bytes::<(u32, W)>(masks.capacity(), staging_bytes, limits.max_working_bytes)?;
        let owned_bytes =
            checked_bytes::<(u32, W)>(masks.capacity(), base, limits.max_working_bytes)?;
        for (symbol, position) in positions {
            if let Some(previous) = masks.last_mut().filter(|last| last.0 == symbol) {
                previous.1 = previous.1.union(W::bit(position));
            } else {
                masks.push((symbol, W::bit(position)));
            }
        }

        Ok(Self {
            masks,
            loops,
            accept,
            length,
            pattern_units,
            owned_bytes,
            limits,
        })
    }
}

fn one_scalar(mut variants: impl Iterator<Item = char>) -> Option<char> {
    let first = variants.next()?;
    variants.next().is_none().then_some(first)
}
