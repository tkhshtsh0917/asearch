//! Reserves reusable scalar rows and checkpoints before matching begins.

use super::{ScalarError, resources::checked_bytes};
use crate::BitWord;
use std::mem::size_of;

/// Reusable states. Create with [`super::ScalarWm::workspace`], then reuse across texts.
#[derive(Debug)]
pub struct ScalarWorkspace<W: BitWord> {
    pub(super) states: Vec<W>,
}

impl<W: BitWord> ScalarWorkspace<W> {
    pub(super) fn new(max_ambig: u16, owned: usize, limit: usize) -> Result<Self, ScalarError> {
        let base = owned
            .checked_add(size_of::<Self>())
            .ok_or(ScalarError::WorkingLimit)?;
        let count = usize::from(max_ambig) + 1;
        checked_bytes::<W>(count, base, limit)?;
        let mut states = Vec::new();
        states
            .try_reserve_exact(count)
            .map_err(|_| ScalarError::Reservation)?;
        checked_bytes::<W>(states.capacity(), base, limit)?;
        states.resize(count, W::zero());
        Ok(Self { states })
    }

    pub fn max_ambig(&self) -> u16 {
        (self.states.len() - 1) as u16
    }

    pub(super) fn check_budget(&self, owned: usize, limit: usize) -> Result<(), ScalarError> {
        let base = owned
            .checked_add(size_of::<Self>())
            .ok_or(ScalarError::WorkingLimit)?;
        checked_bytes::<W>(self.states.capacity(), base, limit).map(|_| ())
    }
}
