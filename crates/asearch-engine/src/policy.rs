//! Ambiguity selection from normalized, unwrapped input.

use crate::{MatchMode, Profile};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LengthUnit {
    #[default]
    Profile,
    CodePoint,
    Utf16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Policy {
    Fixed(u8),
    ByLength {
        every: usize,
        max: u8,
        unit: LengthUnit,
        include_wildcards: bool,
    },
}

/// Validated policy for bulk search (levels 0..=7). Pass normalized input
/// before adding internal wrap loops. Evaluation does not allocate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmbigPolicy(Policy);

impl Default for AmbigPolicy {
    fn default() -> Self {
        Self(Policy::ByLength {
            every: 4,
            max: 3,
            unit: LengthUnit::Profile,
            include_wildcards: false,
        })
    }
}

impl AmbigPolicy {
    pub fn fixed(level: u8) -> Result<Self, InvalidPolicy> {
        if level > 7 {
            return Err(InvalidPolicy);
        }
        Ok(Self(Policy::Fixed(level)))
    }

    pub fn by_length(
        every: usize,
        max: u8,
        unit: LengthUnit,
        include_wildcards: bool,
    ) -> Result<Self, InvalidPolicy> {
        if every == 0 || every as u128 > 9_007_199_254_740_991 || max > 7 {
            return Err(InvalidPolicy);
        }
        Ok(Self(Policy::ByLength {
            every,
            max,
            unit,
            include_wildcards,
        }))
    }

    /// Neither normalizes nor adds wrap itself; callers do those in the
    /// specified order. Lone surrogates count as one symbol in either unit.
    pub fn evaluate(self, normalized: &[u16], mode: MatchMode, profile: Profile) -> u8 {
        let (every, max, unit, include_wildcards) = match self.0 {
            Policy::Fixed(level) => return level,
            Policy::ByLength {
                every,
                max,
                unit,
                include_wildcards,
            } => (every, max, unit, include_wildcards),
        };
        let skip_spaces = mode == MatchMode::WildcardSpace && !include_wildcards;
        let utf16 = unit == LengthUnit::Utf16
            || (unit == LengthUnit::Profile && profile == Profile::AsearchTsLib);
        let length = if utf16 {
            normalized
                .iter()
                .filter(|&&unit| !skip_spaces || unit != 0x20)
                .count()
        } else {
            char::decode_utf16(normalized.iter().copied())
                .filter(|symbol| !skip_spaces || *symbol != Ok(' '))
                .count()
        };
        (length / every).min(usize::from(max)) as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidPolicy;

impl fmt::Display for InvalidPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("policy requires a positive safe-integer period and a level in 0..7")
    }
}

impl Error for InvalidPolicy {}
