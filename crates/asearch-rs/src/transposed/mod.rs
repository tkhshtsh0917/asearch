//! Block-aligned corpus-bit DP. Unlike compatibility matching, levels are
//! monotonic Ideal costs; profiles only select the corpus symbol unit.

mod build;
mod eq;
mod matching;
mod workspace;

use crate::{Engine, Normalizer, Profile, QueryWorkspace, ResourceLimits, SearchError};
use eq::EqTable;
pub use workspace::TransposedWorkspace;

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

#[derive(Clone, Copy, Debug, Default)]
struct Mask {
    sep: u64,
    text: u64,
    end: u64,
    end_before: usize,
}

#[derive(Clone, Copy, Debug)]
struct Block {
    start: usize,
    words: usize,
}

/// Owns derived masks and EQ containers, but no original/display strings.
/// Input order is preserved. Document ranking belongs to the owning index.
#[derive(Debug)]
pub struct TransposedEngine {
    options: TransposedOptions,
    limits: ResourceLimits,
    masks: Vec<Mask>,
    blocks: Vec<Block>,
    eq: EqTable,
    texts: usize,
    max_block_words: usize,
    capacity_bytes: usize,
}

impl TransposedEngine {
    pub fn new(
        texts: &[&[u16]],
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        build::build(texts, options, limits)
    }

    pub fn workspace(
        &self,
        query: &QueryWorkspace,
        max_ambig: u8,
    ) -> Result<TransposedWorkspace, SearchError> {
        workspace::prepare(self, query, max_ambig)
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    /// Includes boundary, padding and the zero guard word.
    pub fn slots(&self) -> usize {
        self.masks.len() * 64
    }

    pub(crate) fn set_working_budget(&mut self, bytes: usize) {
        self.limits.max_working_bytes = bytes;
    }
}

impl Engine for TransposedEngine {
    type Query = QueryWorkspace;
    type Workspace = TransposedWorkspace;
    type Error = SearchError;

    fn search<'w>(
        &self,
        query: &QueryWorkspace,
        workspace: &'w mut TransposedWorkspace,
    ) -> Result<&'w [Option<u16>], SearchError> {
        workspace.validate(self, query)?;
        matching::search(self, query, workspace);
        Ok(&workspace.levels)
    }
}

fn symbols(units: &[u16]) -> impl Iterator<Item = u32> + '_ {
    std::char::decode_utf16(units.iter().copied())
        .map(|c| c.map_or_else(|error| u32::from(error.unpaired_surrogate()), u32::from))
}
