//! Wildcard approximate matching compatible with asearch.ts 1.0.3.
//!
//! `AsearchTs` iterates text by code point; `AsearchTsLib` preserves the
//! published JavaScript's UTF-16 iteration. Neither profile normalizes text.
//!
//! # Matching
//!
//! ```
//! use asearch_rs::{Asearch, MatchMode};
//!
//! let pattern = Asearch::new("hello", MatchMode::Exact)?;
//! assert!(pattern.is_match("helo", 1));
//! # Ok::<(), asearch_rs::PatternTooLong>(())
//! ```
//!
//! # Profiles
//!
//! The published JS compatibility profile intentionally preserves the original
//! non-BMP mismatch. Rust casing is locale independent; use JS-compiled masks
//! with [`Asearch::from_parts`] when exact host-locale behavior is required.
//!
//! ```
//! use asearch_rs::{Asearch, MatchMode, Profile};
//!
//! let modern = Asearch::new("😀", MatchMode::Exact)?;
//! let published = Asearch::with_profile("😀", MatchMode::Exact, Profile::AsearchTsLib)?;
//! assert!(modern.is_match("😀", 0));
//! assert!(!published.is_match("😀", 0));
//! # Ok::<(), asearch_rs::PatternTooLong>(())
//! ```
//!
//! # Compatibility levels
//!
//! A level is not a monotonic maximum edit distance. Increasing it can change a
//! match back to false. Levels above 31 also preserve JS shift wrapping.
//!
//! ```
//! use asearch_rs::{Asearch, MatchMode};
//!
//! let pattern = Asearch::new("a", MatchMode::Exact)?;
//! assert!(!pattern.is_match("", 0));
//! assert!(pattern.is_match("", 1));
//! assert!(!pattern.is_match("", 2));
//! # Ok::<(), asearch_rs::PatternTooLong>(())
//! ```
#![forbid(unsafe_code)]

mod capacity;
mod engine;
mod index;
mod level;
mod matcher;
mod normalize;
mod pattern;
mod policy;
mod scalar;
mod search;
mod transposed;
mod word;

pub use engine::Engine;
pub use index::{
    AsearchIndex, Doc, ExpectedSegment, IndexSet, IndexWorkspace, SearchHit, SearchResults,
    SegmentHit, SegmentInfo, SetResults, SetWorkspace, WorkingReservation,
};
pub use level::{InvalidMaxAmbig, MAX_LEVEL};
pub use normalize::{NormalizeError, Normalizer};
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
