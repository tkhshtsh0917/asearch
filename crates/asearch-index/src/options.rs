//! Defines matching profiles, symbol units and validated corpus layout options.

use crate::{Normalizer, SearchError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum MatchMode {
    #[default]
    Exact = 0,
    Include = 1,
    WildcardSpace = 2,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Profile {
    #[default]
    AsearchTs,
    AsearchTsLib,
    /// Monotonic Ideal initialization, using code-point iteration.
    Ideal,
}

#[derive(Clone, Copy, Debug)]
pub struct TransposedOptions {
    pub normalizer: Normalizer,
    pub profile: Profile,
    pub block_words: usize,
}

impl Default for TransposedOptions {
    fn default() -> Self {
        Self {
            normalizer: Normalizer::default(),
            profile: Profile::Ideal,
            block_words: 512,
        }
    }
}

impl TransposedOptions {
    pub fn validate(self) -> Result<(), SearchError> {
        if self.block_words == 0 {
            return Err(SearchError::InvalidInput("block_words must be positive"));
        }
        self.block_words
            .checked_mul(64)
            .ok_or(SearchError::ResourceLimit("block width overflow"))?;
        Ok(())
    }
}
