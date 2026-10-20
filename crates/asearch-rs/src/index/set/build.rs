use super::{IndexSet, LiveDocument};
use crate::{
    AsearchIndex, ResourceLimits, SearchError,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

pub(super) const ARC_CONTROL: usize = 2 * size_of::<usize>();

pub(super) fn owned(
    mut segments: Vec<AsearchIndex>,
    limits: ResourceLimits,
) -> Result<IndexSet, SearchError> {
    limits.validate()?;
    if segments.is_empty() || segments.len() > limits.max_segments {
        return Err(SearchError::InvalidInput("invalid server segment count"));
    }
    let initial = segments.iter().try_fold(
        add(
            size_of::<IndexSet>(),
            mul(segments.capacity(), size_of::<AsearchIndex>())?,
        )?,
        |used, part| {
            add(
                used,
                part.limits.max_working_bytes - part.available_working_bytes()?,
            )
        },
    )?;
    let mut budget = Budget::new(
        add(initial, mul(segments.len(), ARC_CONTROL)?)?,
        limits.max_working_bytes,
    )?;
    let mut shared = budget.reserve(segments.len())?;
    for mut part in segments.drain(..) {
        part.set_runtime_limits(limits)?;
        if part.source.is_none() {
            let extra = limits.max_working_bytes - budget.used;
            let local = part.limits.max_working_bytes - part.available_working_bytes()?;
            let bytes = super::super::format::write_limited(&part, add(local, extra)?)?;
            part.source = Some(super::super::SourceInfo {
                id: u64::from_le_bytes(bytes[24..32].try_into().expect("encoded header")),
                bytes: bytes.len(),
            });
        }
        shared.push(std::sync::Arc::new(part));
    }
    drop(segments);
    build(shared, limits)
}

pub(super) fn build(
    segments: Vec<std::sync::Arc<AsearchIndex>>,
    limits: ResourceLimits,
) -> Result<IndexSet, SearchError> {
    limits.validate()?;
    if segments.is_empty() || segments.len() > limits.max_segments {
        return Err(SearchError::InvalidInput(
            "expected one to max_segments server segments",
        ));
    }
    let first = segments[0].engine.options();
    let mut initial = add(size_of::<IndexSet>(), super::super::reservations::OVERHEAD)?;
    initial = add(
        initial,
        mul(
            segments.capacity(),
            size_of::<std::sync::Arc<AsearchIndex>>(),
        )?,
    )?;
    initial = add(initial, mul(segments.len() + 1, ARC_CONTROL)?)?;
    initial = add(initial, size_of::<std::sync::atomic::AtomicUsize>())?;
    let mut texts = 0;
    let mut units = 0;
    let mut slots = 0;
    let mut input = 0;
    let mut documents = 0;
    let mut offsets = [0; 9];
    for (i, segment) in segments.iter().enumerate() {
        let options = segment.engine.options();
        if options.normalizer != first.normalizer || options.profile != first.profile {
            return Err(SearchError::InvalidIndex("segment descriptors differ"));
        }
        let source = segment
            .source
            .ok_or(SearchError::InvalidIndex("missing source descriptor"))?;
        if segments[..i]
            .iter()
            .any(|old| old.source.is_some_and(|s| s.id == source.id))
        {
            return Err(SearchError::InvalidIndex("duplicate segment ID"));
        }
        offsets[i] = documents;
        documents = add(documents, segment.documents.len())?;
        texts = add(texts, segment.text_count())?;
        units = add(units, segment.normalized.len())?;
        slots = add(slots, segment.engine.slots())?;
        input = add(
            input,
            add(
                mul(
                    add(segment.original.capacity(), segment.normalized.capacity())?,
                    2,
                )?,
                mul(segment.tombstones.capacity(), 4)?,
            )?,
        )?;
        initial = add(
            initial,
            segment.limits.max_working_bytes - segment.available_working_bytes()?,
        )?;
    }
    offsets[segments.len()] = documents;
    if texts > limits.max_texts
        || units > limits.max_units
        || slots > limits.max_slots
        || input > limits.max_total_bytes
    {
        return Err(SearchError::ResourceLimit(
            "aggregate segment count or input budget",
        ));
    }
    let mut budget = Budget::new(initial, limits.max_working_bytes)?;
    let mut masks = budget.reserve(documents)?;
    masks.resize(documents, 0);
    let mut live = budget.reserve(documents)?;
    for (s, segment) in segments.iter().enumerate() {
        for (i, doc) in segment.documents.iter().enumerate() {
            if !segments[s + 1..]
                .iter()
                .any(|later| later.tombstones.binary_search(&doc.key).is_ok())
            {
                live.push(LiveDocument {
                    key: doc.key,
                    order: doc.order,
                    segment: s,
                    document: i,
                });
            }
        }
    }
    live.sort_unstable_by_key(|doc| doc.key);
    if live.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(SearchError::InvalidIndex(
            "multiple live segments contain the same key",
        ));
    }
    let has_original = live
        .first()
        .is_none_or(|doc| segments[doc.segment].has_original);
    if live
        .iter()
        .any(|doc| segments[doc.segment].has_original != has_original)
    {
        return Err(SearchError::InvalidIndex(
            "live documents mix present and absent originals",
        ));
    }
    for doc in &live {
        masks[offsets[doc.segment] + doc.document] = 1;
    }
    live.sort_unstable_by_key(|doc| (doc.order, doc.key));
    let existing_leases = segments.iter().try_fold(0usize, |used, segment| {
        add(
            used,
            segment.limits.max_working_bytes
                - segment.available_working_bytes()?
                - segment.capacity_bytes,
        )
    })?;
    Ok(IndexSet {
        segments,
        live,
        masks,
        offsets,
        limits,
        has_original,
        capacity_bytes: budget.used - existing_leases,
        reservations: Default::default(),
        activity: Default::default(),
        revision: 1,
    })
}
