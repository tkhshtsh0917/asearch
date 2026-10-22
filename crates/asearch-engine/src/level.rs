//! Validates ambiguity levels shared by native matching entry points.

use crate::{Asearch, Profile};
use std::{error::Error, fmt};

/// Maximum level for minimum-level evaluation, before JS shift wrapping.
pub const MAX_LEVEL: u16 = 31;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InvalidMaxAmbig(pub u16);

impl fmt::Display for InvalidMaxAmbig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "max_ambig must be <= {MAX_LEVEL}, got {}", self.0)
    }
}

impl Error for InvalidMaxAmbig {}

impl Asearch {
    /// Returns the first matching level, preserving the selected profile.
    /// Levels above 31 are rejected instead of introducing JS shift wrapping.
    pub fn match_level(&self, text: &str, max_ambig: u16) -> Result<Option<u16>, InvalidMaxAmbig> {
        match self.profile {
            Profile::AsearchTs | Profile::Ideal => {
                self.level_symbols(text.chars().map(u32::from), max_ambig)
            }
            Profile::AsearchTsLib => {
                self.level_symbols(text.encode_utf16().map(u32::from), max_ambig)
            }
        }
    }

    pub fn match_level_utf16(
        &self,
        text: &[u16],
        max_ambig: u16,
    ) -> Result<Option<u16>, InvalidMaxAmbig> {
        match self.profile {
            Profile::AsearchTs | Profile::Ideal => self.level_symbols(
                char::decode_utf16(text.iter().copied())
                    .map(|c| c.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from)),
                max_ambig,
            ),
            Profile::AsearchTsLib => {
                self.level_symbols(text.iter().copied().map(u32::from), max_ambig)
            }
        }
    }

    fn level_symbols(
        &self,
        text: impl Iterator<Item = u32>,
        max_ambig: u16,
    ) -> Result<Option<u16>, InvalidMaxAmbig> {
        if max_ambig > MAX_LEVEL {
            return Err(InvalidMaxAmbig(max_ambig));
        }
        let mut stack = [0u32; 32];
        let states = &mut stack[..usize::from(max_ambig) + 1];
        self.advance(text, states);
        Ok(states
            .iter()
            .position(|state| state & self.accept != 0)
            .map(|level| level as u16))
    }
}
