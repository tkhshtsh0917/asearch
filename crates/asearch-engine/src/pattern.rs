//! Compiles compatibility patterns and exposes single-text matching APIs.

use crate::MAX_LENGTH;
use std::{collections::BTreeMap, error::Error, fmt};

pub use asearch_index::{MatchMode, Profile};

#[derive(Debug, Eq, PartialEq)]
pub struct PatternTooLong {
    pub pattern: String,
}

impl fmt::Display for PatternTooLong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pattern must be shorter than 32 chars. But given {}",
            self.pattern
        )
    }
}

impl Error for PatternTooLong {}

#[derive(Debug)]
pub struct Asearch {
    pub(crate) masks: Vec<(u32, u32)>,
    pub(crate) loop_mask: u32,
    pub(crate) accept: u32,
    pub(crate) init: u32,
    pub(crate) profile: Profile,
}

impl Asearch {
    pub fn new(pattern: &str, mode: MatchMode) -> Result<Self, PatternTooLong> {
        Self::with_profile(pattern, mode, Profile::default())
    }

    /// Uses Rust Unicode casing without locale selection. Use `from_parts`
    /// with JavaScript-generated masks for the host's exact locale behavior.
    pub fn with_profile(
        pattern: &str,
        mode: MatchMode,
        profile: Profile,
    ) -> Result<Self, PatternTooLong> {
        if pattern.encode_utf16().count() > MAX_LENGTH {
            return Err(PatternTooLong {
                pattern: pattern.into(),
            });
        }

        let init = 1u32 << 31;
        let mut accept = init.wrapping_shr(pattern.chars().count() as u32);
        let mut loop_mask = if mode == MatchMode::Include {
            init | accept
        } else {
            0
        };
        let mut masks = BTreeMap::new();
        let mut position = 0;
        for c in pattern.chars() {
            if mode == MatchMode::WildcardSpace && c == ' ' {
                loop_mask |= init.wrapping_shr(position);
                accept = accept.wrapping_shl(1);
                continue;
            }
            let bit = init.wrapping_shr(position);
            *masks.entry(u32::from(c)).or_insert(0) |= bit;
            for mut variants in [
                c.to_lowercase().collect::<Vec<_>>(),
                c.to_uppercase().collect(),
            ] {
                if variants.len() == 1 {
                    *masks.entry(u32::from(variants.remove(0))).or_insert(0) |= bit;
                }
            }
            position += 1;
        }
        Ok(Self::from_parts(
            &masks.into_iter().collect::<Vec<_>>(),
            loop_mask,
            accept,
            init,
            profile,
        ))
    }

    /// Constructs a matcher from already compiled bit masks. Duplicate symbols
    /// are merged; the table is sorted once, outside the matching hot path.
    pub fn from_parts(
        masks: &[(u32, u32)],
        loop_mask: u32,
        accept: u32,
        init: u32,
        profile: Profile,
    ) -> Self {
        let mut table = BTreeMap::new();
        for &(symbol, mask) in masks {
            *table.entry(symbol).or_insert(0) |= mask;
        }
        Self {
            masks: table.into_iter().collect(),
            loop_mask,
            accept,
            init,
            profile,
        }
    }
}
