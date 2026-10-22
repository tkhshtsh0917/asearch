//! Exports bounded query workspaces and the shared search error contract.

pub(crate) mod classes;
mod query;

pub use asearch_index::{ResourceLimits, SearchError};
pub use query::{QueryOptions, QueryWorkspace};
