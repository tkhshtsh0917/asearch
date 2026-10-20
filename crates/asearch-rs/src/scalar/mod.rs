//! Resource-bounded scalar Ideal matching, separate from JS compatibility.

mod compiler;
mod engine;
mod matching;
mod resources;
mod workspace;

pub use engine::{ScalarEngine, ScalarEngineWorkspace};
pub use resources::{ScalarError, ScalarLimits};
pub use workspace::ScalarWorkspace;

use crate::BitWord;

/// An Ideal matcher over Unicode scalar values, with locale-independent casing.
/// No normalization is performed. ASCII spaces in WildcardSpace are loops.
///
/// ```
/// use asearch_rs::{MatchMode, MultiWord, ScalarLimits, ScalarWm};
///
/// let pattern = "検索".repeat(40);
/// let matcher = ScalarWm::<MultiWord<2>>::new(
///     &pattern, MatchMode::Exact, ScalarLimits::default(),
/// )?;
/// let mut workspace = matcher.workspace(3)?;
/// assert_eq!(matcher.match_level(&pattern, &mut workspace)?, Some(0));
/// # Ok::<(), asearch_rs::ScalarError>(())
/// ```
#[derive(Debug)]
pub struct ScalarWm<W: BitWord> {
    masks: Vec<(u32, W)>,
    loops: W,
    accept: W,
    length: usize,
    pattern_units: usize,
    owned_bytes: usize,
    limits: ScalarLimits,
}

impl<W: BitWord> ScalarWm<W> {
    /// Reserves one workspace. The matcher and this workspace share the budget.
    pub fn workspace(&self, max_ambig: u16) -> Result<ScalarWorkspace<W>, ScalarError> {
        ScalarWorkspace::new(max_ambig, self.owned_bytes, self.limits.max_working_bytes)
    }
}
