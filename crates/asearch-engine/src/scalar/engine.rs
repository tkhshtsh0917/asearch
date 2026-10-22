//! Owns scalar corpus data and exposes reusable search workspaces.

use super::{ScalarError, ScalarLimits, ScalarWm, ScalarWorkspace, resources::checked_bytes};
use crate::{BitWord, Engine};
use std::{marker::PhantomData, mem::size_of};

/// Scalar scan of a borrowed corpus. Text storage belongs to the caller and
/// is excluded from this adapter's capacity budget. There is no normalization.
///
/// ```
/// use asearch_engine::{Engine, MatchMode, ScalarEngine, ScalarLimits, ScalarWm};
///
/// let texts = ["hello", "helo", "unrelated"];
/// let limits = ScalarLimits::default();
/// let query = ScalarWm::<u64>::new("hello", MatchMode::Exact, limits)?;
/// let engine = ScalarEngine::new(&texts, limits)?;
/// let mut workspace = engine.workspace(&query, 1)?;
/// assert_eq!(engine.search(&query, &mut workspace)?, &[Some(0), Some(1), None]);
/// # Ok::<(), asearch_engine::ScalarError>(())
/// ```
#[derive(Debug)]
pub struct ScalarEngine<'a, W: BitWord> {
    texts: &'a [&'a str],
    limits: ScalarLimits,
    word: PhantomData<W>,
}

/// Reserved scalar states and one output slot per corpus text.
#[derive(Debug)]
pub struct ScalarEngineWorkspace<W: BitWord> {
    scalar: ScalarWorkspace<W>,
    levels: Vec<Option<u16>>,
}

impl<'a, W: BitWord> ScalarEngine<'a, W> {
    pub fn new(texts: &'a [&'a str], limits: ScalarLimits) -> Result<Self, ScalarError> {
        limits.validate()?;
        Ok(Self {
            texts,
            limits,
            word: PhantomData,
        })
    }

    pub fn workspace(
        &self,
        query: &ScalarWm<W>,
        max_ambig: u16,
    ) -> Result<ScalarEngineWorkspace<W>, ScalarError> {
        let limit = self.byte_limit(query);
        let base = self.base_bytes(query)?;
        let output_bytes = checked_bytes::<Option<u16>>(self.texts.len(), base, limit)?;
        checked_bytes::<W>(usize::from(max_ambig) + 1, output_bytes, limit)?;

        let mut levels = Vec::new();
        levels
            .try_reserve_exact(self.texts.len())
            .map_err(|_| ScalarError::Reservation)?;
        let output_bytes = checked_bytes::<Option<u16>>(levels.capacity(), base, limit)?;
        // base already contains the ScalarWorkspace header through the outer
        // workspace; its constructor accounts for that header once itself.
        let scalar = ScalarWorkspace::new(
            max_ambig,
            output_bytes - size_of::<ScalarWorkspace<W>>(),
            limit,
        )?;
        levels.resize(self.texts.len(), None);
        Ok(ScalarEngineWorkspace { scalar, levels })
    }

    fn byte_limit(&self, query: &ScalarWm<W>) -> usize {
        self.limits
            .max_working_bytes
            .min(query.limits.max_working_bytes)
    }

    fn base_bytes(&self, query: &ScalarWm<W>) -> Result<usize, ScalarError> {
        if query.pattern_units > self.limits.max_pattern_units {
            return Err(ScalarError::PatternLimit);
        }
        query
            .owned_bytes
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(size_of::<ScalarEngineWorkspace<W>>()))
            .ok_or(ScalarError::WorkingLimit)
    }

    fn validate_workspace(
        &self,
        query: &ScalarWm<W>,
        workspace: &ScalarEngineWorkspace<W>,
    ) -> Result<(), ScalarError> {
        if workspace.levels.len() < self.texts.len() {
            return Err(ScalarError::WorkspaceCapacity);
        }
        let limit = self.byte_limit(query);
        let bytes = checked_bytes::<Option<u16>>(
            workspace.levels.capacity(),
            self.base_bytes(query)?,
            limit,
        )?;
        checked_bytes::<W>(workspace.scalar.states.capacity(), bytes, limit)?;
        Ok(())
    }
}

impl<W: BitWord> Engine for ScalarEngine<'_, W> {
    type Query = ScalarWm<W>;
    type Workspace = ScalarEngineWorkspace<W>;
    type Error = ScalarError;

    fn search<'w>(
        &self,
        query: &Self::Query,
        workspace: &'w mut Self::Workspace,
    ) -> Result<&'w [Option<u16>], Self::Error> {
        // Validation precedes writes, preserving previous output on failure.
        self.validate_workspace(query, workspace)?;
        for (text, level) in self.texts.iter().zip(&mut workspace.levels) {
            *level = query.match_level(text, &mut workspace.scalar)?;
        }
        Ok(&workspace.levels[..self.texts.len()])
    }
}
