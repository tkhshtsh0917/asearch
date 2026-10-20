//! Owned immutable ASIX segment. Tombstones affect older segments only;
//! this single-segment search keeps its own replacement texts live.

mod build;
mod delta;
mod format;
mod integrity;
pub use integrity::ExpectedSegment;
mod narrowing;
mod reservations;
mod set;
mod workspace;
pub use set::{
    IndexSet, PreparedUpdate, SegmentHit, SegmentInfo, SetResults, SetWorkspace, WorkingReservation,
};

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

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceInfo {
    id: u64,
    bytes: usize,
}

/// Owns originals, normalized UTF-16 and derived corpus-bit structures.
#[derive(Debug)]
pub struct AsearchIndex {
    source: Option<SourceInfo>,
    reservations: std::sync::Arc<reservations::Reservations>,
    engine: TransposedEngine,
    tombstones: Vec<u32>,
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

    /// Builds one immutable server delta. Tombstones apply only to older
    /// segments; texts in this same segment remain live.
    pub fn from_delta(
        docs: &[Doc<'_>],
        tombstones: &[u32],
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        delta::build(docs, tombstones, options, limits)
    }

    pub fn tombstones(&self) -> &[u32] {
        &self.tombstones
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
        self.search_compiled(query, None, workspace, limit, count_total, None)
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
        self.search_compiled(query, Some(keys), workspace, limit, count_total, None)
    }

    fn search_compiled<'w>(
        &self,
        query: &QueryWorkspace,
        keys: Option<&[u32]>,
        workspace: &'w mut IndexWorkspace,
        limit: usize,
        count_total: bool,
        live: Option<&[u8]>,
    ) -> Result<SearchResults<'w>, SearchError> {
        if live.is_some_and(|mask| mask.len() != self.documents.len()) {
            return Err(SearchError::InvalidInput("invalid live document mask"));
        }
        workspace.validate(self, query)?;
        workspace.engine.validate(&self.engine, query)?;
        if let Some(keys) = keys {
            crate::transposed::casing::validate(query, keys)?;
        }
        if workspace.narrowing.is_some() {
            narrowing::compile(self, query, &mut workspace.engine, keys);
        }
        let narrow = workspace
            .narrowing
            .as_ref()
            .is_some_and(|planner| planner.should_use(self, query, &workspace.engine));
        if narrow {
            workspace
                .narrowing
                .as_mut()
                .expect("planner checked above")
                .search(self, query, &mut workspace.engine);
        } else if let Some(cache) = &mut workspace.incremental {
            cache.search(&self.engine, query, &mut workspace.engine, keys);
        } else if let Some(keys) = keys {
            self.engine
                .search_with_casing(query, keys, &mut workspace.engine)?;
        } else {
            self.engine.search(query, &mut workspace.engine)?;
        }
        if let Some(planner) = &mut workspace.narrowing {
            planner.remember(self, query, &workspace.engine);
        }
        let levels = workspace.engine.levels();
        Ok(workspace::extract(
            &self.documents,
            levels,
            query.max_ambig(),
            &mut workspace.output,
            limit,
            count_total,
            live,
        ))
    }

    /// Remaining capacity after all registered query/workspace leases.
    pub fn available_working_bytes(&self) -> Result<usize, SearchError> {
        let live = self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("workspace accounting unavailable"))?;
        Ok(self
            .limits
            .max_working_bytes
            .saturating_sub(crate::capacity::add(self.capacity_bytes, *live)?))
    }

    fn set_runtime_limits(&mut self, limits: ResourceLimits) -> Result<(), SearchError> {
        let own = self.capacity_bytes - self.engine.capacity_bytes();
        let remaining = limits
            .max_working_bytes
            .checked_sub(own)
            .filter(|&n| n > 0)
            .ok_or(SearchError::ResourceLimit("segment runtime budget"))?;
        self.limits = limits;
        self.engine.set_runtime_limits(ResourceLimits {
            max_working_bytes: remaining,
            ..limits
        });
        Ok(())
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

    /// Canonical ASIX v1 segment. No timestamps, machine paths or derived
    /// layout data enter the byte stream.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SearchError> {
        format::write(self)
    }

    /// Exact ASIX size without allocating the serialized buffer.
    pub fn serialized_size(&self) -> Result<usize, SearchError> {
        format::size(self)
    }

    /// Validates an untrusted server segment before constructing its runtime
    /// layout. The input is borrowed; its allocation belongs to the caller.
    pub fn from_bytes(
        bytes: &[u8],
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        format::load(bytes, options, limits, 0)
    }

    /// Verifies a trusted manifest descriptor before building derived arrays.
    pub fn from_bytes_expected(
        bytes: &[u8],
        options: TransposedOptions,
        limits: ResourceLimits,
        expected: &ExpectedSegment,
    ) -> Result<Self, SearchError> {
        limits.validate()?;
        if bytes.len() > limits.max_total_bytes {
            return Err(SearchError::ResourceLimit("ASIX input budget"));
        }
        expected.verify(bytes)?;
        Self::from_bytes(bytes, options, limits)
    }

    pub fn from_owned_bytes_expected(
        bytes: Vec<u8>,
        options: TransposedOptions,
        limits: ResourceLimits,
        expected: &ExpectedSegment,
    ) -> Result<Self, SearchError> {
        limits.validate()?;
        if bytes.capacity() > limits.max_total_bytes || bytes.capacity() > limits.max_working_bytes
        {
            return Err(SearchError::ResourceLimit("ASIX owned input budget"));
        }
        expected.verify(&bytes)?;
        Self::from_owned_bytes(bytes, options, limits)
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
