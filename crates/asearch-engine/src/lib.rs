//! Wildcard approximate matching, indexed search and bounded query workspaces.
//!
//! Exposes scalar and corpus matching, normalization, segment updates and explanations.

#![forbid(unsafe_code)]

use asearch_index::capacity;
mod engine;
mod explain;
mod index;
mod intent;
mod level;
mod matcher;

mod pattern;
mod policy;
mod scalar;
mod search;
mod transposed;
mod word;

pub use asearch_index::{NormalizeError, Normalizer};
pub use engine::Engine;
pub use explain::{CoordinateSpace, Explanation};
pub use index::{
    AsearchIndex, Doc, ExpectedSegment, IndexSet, IndexWorkspace, PreparedUpdate, SearchHit,
    SearchResults, SegmentHit, SegmentInfo, SetResults, SetWorkspace, WorkingReservation,
};
pub use intent::{IntentEntry, IntentHit, IntentIndex, IntentResults};
pub use level::{InvalidMaxAmbig, MAX_LEVEL};
pub use pattern::{Asearch, MatchMode, PatternTooLong, Profile};
pub use policy::{AmbigPolicy, InvalidPolicy, LengthUnit};
pub use scalar::{
    ScalarEngine, ScalarEngineWorkspace, ScalarError, ScalarLimits, ScalarWm, ScalarWorkspace,
};
pub use search::{QueryOptions, QueryWorkspace, ResourceLimits, SearchError};
pub use transposed::{EngineStats, TransposedEngine, TransposedOptions, TransposedWorkspace};
pub use word::{BitWord, MultiWord};

pub const MAX_LENGTH: usize = 31;
pub const MAX_AMBIG: u16 = u16::MAX;
