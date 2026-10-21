//! Immutable ordered server segments. Updates build a new validated column;
//! no client-side edit layer exists.
mod build;
mod update;
mod workspace;

use super::{AsearchIndex, reservations::Reservations};
use crate::{QueryWorkspace, ResourceLimits, SearchError, capacity::add};
use std::sync::{Arc, atomic::AtomicUsize};
pub use workspace::{SegmentHit, SetResults, SetWorkspace};

#[derive(Debug)]
pub struct WorkingReservation(pub(crate) super::reservations::Lease);
impl WorkingReservation {
    pub fn capacity_bytes(&self) -> usize {
        self.0.bytes()
    }
}
/// Holds exclusive access to the old column until all dependent wrapper
/// resources are ready. Dropping this guard cancels without adopting.
pub struct PreparedUpdate<'a> {
    active: &'a mut IndexSet,
    candidate: IndexSet,
    limits: ResourceLimits,
}
impl PreparedUpdate<'_> {
    pub fn text_count(&self) -> usize {
        self.candidate.text_count()
    }
    pub fn revision(&self) -> u32 {
        self.candidate.revision()
    }
    pub fn reserve_working(&self, bytes: usize) -> Result<WorkingReservation, SearchError> {
        self.candidate.reserve_working(bytes)
    }
    pub fn commit(mut self) {
        self.candidate.limits = self.limits;
        *self.active = self.candidate;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentInfo {
    pub id: u64,
    pub bytes: usize,
    pub texts: usize,
    pub dead: usize,
}

#[derive(Clone, Copy, Debug)]
struct LiveDocument {
    key: u32,
    order: u32,
    segment: usize,
    document: usize,
}

#[derive(Debug)]
pub struct IndexSet {
    segments: Vec<Arc<AsearchIndex>>,
    live: Vec<LiveDocument>,
    masks: Vec<u8>,
    offsets: [usize; 9],
    limits: ResourceLimits,
    #[cfg(feature = "builder")]
    has_original: bool,
    capacity_bytes: usize,
    reservations: Arc<Reservations>,
    activity: Arc<AtomicUsize>,
    revision: u32,
}
impl IndexSet {
    pub fn new(segments: Vec<AsearchIndex>, limits: ResourceLimits) -> Result<Self, SearchError> {
        build::owned(segments, limits)
    }

    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Reserves wrapper-owned buffers independently of query workspaces.
    pub fn reserve_working(&self, bytes: usize) -> Result<WorkingReservation, SearchError> {
        let mut own = self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("set accounting unavailable"))?;
        let required = add(add(self.base_live_bytes()?, *own)?, bytes)?;
        if required > self.limits.max_working_bytes {
            return Err(SearchError::ResourceLimit("external working reservation"));
        }
        *own = add(*own, bytes)?;
        Ok(WorkingReservation(super::reservations::Lease::new(
            self.reservations.clone(),
            bytes,
        )))
    }

    pub fn segments(&self) -> impl Iterator<Item = SegmentInfo> + '_ {
        self.segments.iter().enumerate().map(|(i, part)| {
            let source = part.source.expect("set source metadata");
            let live_texts: usize = self
                .live
                .iter()
                .filter(|doc| doc.segment == i)
                .map(|entry| {
                    let doc = part.documents[entry.document];
                    doc.end - doc.start
                })
                .sum();
            SegmentInfo {
                id: source.id,
                bytes: source.bytes,
                texts: part.text_count(),
                dead: part.text_count() - live_texts,
            }
        })
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn text_count(&self) -> usize {
        self.segments.iter().map(|part| part.text_count()).sum()
    }

    /// A retained text's column-wide position, valid only for this revision.
    pub fn text_id(&self, segment: usize, local: usize) -> Option<usize> {
        if local >= self.segments.get(segment)?.text_count() {
            return None;
        }
        Some(
            self.segments[..segment]
                .iter()
                .map(|part| part.text_count())
                .sum::<usize>()
                + local,
        )
    }

    pub fn text_location(&self, mut flat: usize) -> Option<(usize, usize)> {
        for (segment, part) in self.segments.iter().enumerate() {
            if flat < part.text_count() {
                return Some((segment, flat));
            }
            flat -= part.text_count();
        }
        None
    }

    pub fn document_count(&self) -> usize {
        self.live.len()
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    fn base_live_bytes(&self) -> Result<usize, SearchError> {
        let mut used = self.capacity_bytes;
        for segment in &self.segments {
            let live = segment.limits.max_working_bytes
                - segment.available_working_bytes()?
                - segment.capacity_bytes;
            used = add(used, live)?;
        }
        Ok(used)
    }

    pub fn available_working_bytes(&self) -> Result<usize, SearchError> {
        let own = self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("set accounting unavailable"))?;
        Ok(self
            .limits
            .max_working_bytes
            .saturating_sub(add(self.base_live_bytes()?, *own)?))
    }

    pub fn workspace(
        &self,
        query: &QueryWorkspace,
        max_ambig: u8,
    ) -> Result<SetWorkspace, SearchError> {
        workspace::prepare(self, query, max_ambig)
    }

    pub fn search<'w>(
        &self,
        query: &QueryWorkspace,
        workspace: &'w mut SetWorkspace,
        limit: usize,
        count_total: bool,
    ) -> Result<SetResults<'w>, SearchError> {
        workspace::search(self, query, None, workspace, limit, count_total)
    }

    pub fn text(&self, segment: usize, text_id: usize) -> Option<&[u16]> {
        self.segments.get(segment)?.text(text_id)
    }

    pub fn original_text(&self, segment: usize, text_id: usize) -> Option<&[u16]> {
        self.segments.get(segment)?.original_text(text_id)
    }

    /// Explains a current live text; this bounded display path allocates.
    pub fn explain(
        &self,
        segment: usize,
        text_id: usize,
        query: &QueryWorkspace,
        casing: Option<&[u32]>,
        original: Option<&[u16]>,
    ) -> Result<Option<crate::Explanation>, SearchError> {
        if query.input_units() > self.limits.max_pattern_units
            || query.normalized().len() > self.limits.max_pattern_units
        {
            return Err(SearchError::ResourceLimit(
                "query exceeds set pattern limit",
            ));
        }
        let part = self
            .segments
            .get(segment)
            .ok_or(SearchError::InvalidInput("invalid segment reference"))?;
        if query.options().normalizer != part.engine.options().normalizer
            || query.options().profile != part.engine.options().profile
        {
            return Err(SearchError::InvalidInput(
                "query and corpus descriptors differ",
            ));
        }
        let live = self.live.iter().any(|entry| {
            entry.segment == segment && {
                let doc = part.documents[entry.document];
                (doc.start..doc.end).contains(&text_id)
            }
        });
        if !live {
            return Err(SearchError::InvalidInput("text reference is not live"));
        }
        let normalized = part
            .text(text_id)
            .ok_or(SearchError::InvalidInput("invalid text reference"))?;
        let _query = self.reserve_working(query.capacity_bytes())?;
        let available = self.available_working_bytes()?;
        let _operation = self.reserve_working(available)?;
        crate::explain::explain(
            normalized,
            original.or_else(|| part.original_text(text_id)),
            query,
            casing,
            available,
        )
    }

    pub fn search_with_casing<'w>(
        &self,
        query: &QueryWorkspace,
        keys: &[u32],
        workspace: &'w mut SetWorkspace,
        limit: usize,
        count_total: bool,
    ) -> Result<SetResults<'w>, SearchError> {
        workspace::search(self, query, Some(keys), workspace, limit, count_total)
    }

    /// Writes only live rows directly, preserving originals and canonical
    /// order without constructing another engine or retaining tombstones.
    #[cfg(feature = "builder")]
    pub fn to_bytes(&self) -> Result<Vec<u8>, SearchError> {
        use super::format::encoding::{Encoding, Row, encode};
        let own = self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("set accounting unavailable"))?;
        let rows = self.live.iter().flat_map(|entry| {
            let segment = &self.segments[entry.segment];
            let doc = segment.documents[entry.document];
            (doc.start..doc.end).map(move |i| Row {
                normalized: segment.text(i).expect("live text"),
                original: segment.original_text(i),
                key: entry.key,
                order: entry.order,
            })
        });
        let bytes = encode(
            rows,
            &[],
            Encoding {
                options: self.segments[0].engine.options(),
                has_original: self.has_original,
                limits: self.limits,
                live_bytes: add(self.base_live_bytes()?, *own)?,
            },
        );
        drop(own);
        bytes
    }
}
