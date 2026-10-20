//! Owned, single-segment document index. Binary persistence and server segment
//! updates are separate milestones, not implied by this in-memory API.

mod build;
mod format;
mod workspace;

use crate::{
    Engine, QueryWorkspace, ResourceLimits, SearchError, TransposedEngine, TransposedOptions,
};
pub use workspace::{IndexWorkspace, SearchHit, SearchResults};

/// All texts for one stable document key. A document must contain at least one
/// text; an empty text itself is valid. Keys must be unique within a build.
#[derive(Clone, Copy, Debug)]
pub struct Doc<'a> {
    pub key: u32,
    pub order: u32,
    pub texts: &'a [&'a [u16]],
}

#[derive(Clone, Copy, Debug)]
struct Document {
    key: u32,
    order: u32,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug)]
struct Text {
    original_start: usize,
    original_len: usize,
    normalized_start: usize,
    normalized_len: usize,
}

/// Owns originals, normalized UTF-16 and derived corpus-bit structures.
#[derive(Debug)]
pub struct AsearchIndex {
    engine: TransposedEngine,
    documents: Vec<Document>,
    texts: Vec<Text>,
    original: Vec<u16>,
    normalized: Vec<u16>,
    limits: ResourceLimits,
    capacity_bytes: usize,
    has_original: bool,
}

impl AsearchIndex {
    pub fn from_docs(
        docs: &[Doc<'_>],
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        build::build(docs, options, limits)
    }

    pub fn workspace(
        &self,
        query: &QueryWorkspace,
        max_ambig: u8,
    ) -> Result<IndexWorkspace, SearchError> {
        workspace::prepare(self, query, max_ambig)
    }

    /// Results borrow this workspace until its next successful search. Sorting
    /// and key deduplication do not allocate in the hot path.
    pub fn search<'w>(
        &self,
        query: &QueryWorkspace,
        workspace: &'w mut IndexWorkspace,
        limit: usize,
        count_total: bool,
    ) -> Result<SearchResults<'w>, SearchError> {
        self.search_compiled(query, None, workspace, limit, count_total)
    }

    /// Uses validated host-compiled (original, lower, upper) key triples per
    /// wrapped pattern code point. Native search uses Rust casing by default.
    pub fn search_with_casing<'w>(
        &self,
        query: &QueryWorkspace,
        keys: &[u32],
        workspace: &'w mut IndexWorkspace,
        limit: usize,
        count_total: bool,
    ) -> Result<SearchResults<'w>, SearchError> {
        self.search_compiled(query, Some(keys), workspace, limit, count_total)
    }

    fn search_compiled<'w>(
        &self,
        query: &QueryWorkspace,
        keys: Option<&[u32]>,
        workspace: &'w mut IndexWorkspace,
        limit: usize,
        count_total: bool,
    ) -> Result<SearchResults<'w>, SearchError> {
        workspace.validate(self, query)?;
        let levels = if let Some(keys) = keys {
            self.engine
                .search_with_casing(query, keys, &mut workspace.engine)?
        } else {
            self.engine.search(query, &mut workspace.engine)?
        };
        Ok(workspace::extract(
            &self.documents,
            levels,
            query.max_ambig(),
            &mut workspace.output,
            limit,
            count_total,
        ))
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    pub fn text_count(&self) -> usize {
        self.texts.len()
    }

    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Text IDs are segment-local positions in (order,key,input-text) order.
    pub fn text(&self, text_id: usize) -> Option<&[u16]> {
        let text = self.texts.get(text_id)?;
        Some(&self.normalized[text.normalized_start..text.normalized_start + text.normalized_len])
    }

    pub fn original_text(&self, text_id: usize) -> Option<&[u16]> {
        if !self.has_original {
            return None;
        }
        let text = self.texts.get(text_id)?;
        Some(&self.original[text.original_start..text.original_start + text.original_len])
    }

    /// Canonical ASIX v1 base segment. No timestamps, machine paths or derived
    /// layout data enter the byte stream.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SearchError> {
        format::write(self)
    }

    /// Exact ASIX size without allocating the serialized buffer.
    pub fn serialized_size(&self) -> Result<usize, SearchError> {
        format::size(self)
    }

    /// Validates an untrusted base segment before constructing its runtime
    /// layout. The input is borrowed; its allocation belongs to the caller.
    pub fn from_bytes(
        bytes: &[u8],
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        format::load(bytes, options, limits, 0)
    }

    /// Consumes a transport buffer and includes its allocation in peak input
    /// and working budgets while constructing the candidate index.
    pub fn from_owned_bytes(
        bytes: Vec<u8>,
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        format::load(&bytes, options, limits, bytes.capacity())
    }
}
