//! Encodes already-normalized canonical rows. Compaction can reuse this path
//! without allocating another search engine or fabricating missing originals.
use super::checksum;
use crate::{
    Profile, ResourceLimits, SearchError, TransposedOptions,
    capacity::{Budget, add, mul},
};

#[derive(Clone, Copy)]
pub(crate) struct Row<'a> {
    pub normalized: &'a [u16],
    pub original: Option<&'a [u16]>,
    pub key: u32,
    pub order: u32,
}
#[derive(Clone, Copy)]
pub(crate) struct Encoding {
    pub options: TransposedOptions,
    pub has_original: bool,
    pub limits: ResourceLimits,
    pub live_bytes: usize,
}
struct Layout {
    sections: usize,
    header: usize,
    texts: usize,
    units: usize,
    lengths: [usize; 7],
    offsets: [usize; 7],
    total: usize,
    tomb_index: usize,
}
fn layout<'a>(
    rows: impl Iterator<Item = Row<'a>>,
    tombstones: &[u32],
    settings: Encoding,
) -> Result<Layout, SearchError> {
    let mut texts = 0;
    let mut units = 0;
    let mut originals = 0;
    for row in rows {
        if row.normalized.len() > u16::MAX as usize
            || row.original.is_some_and(|s| s.len() > u16::MAX as usize)
            || row.original.is_some() != settings.has_original
        {
            return Err(SearchError::InvalidInput("inconsistent serialization rows"));
        }
        texts = add(texts, 1)?;
        units = add(units, row.normalized.len())?;
        originals = add(originals, row.original.map_or(0, <[u16]>::len))?;
    }
    let base = if settings.has_original { 6 } else { 4 };
    let sections = base + usize::from(!tombstones.is_empty());
    let header = 64 + sections * 16;
    let mut lengths = [
        mul(units, 2)?,
        mul(texts, 2)?,
        mul(texts, 4)?,
        mul(texts, 4)?,
        mul(texts, 2)?,
        mul(originals, 2)?,
        0,
    ];
    if !tombstones.is_empty() {
        lengths[base] = mul(tombstones.len(), 4)?;
    }
    let mut offsets = [0; 7];
    let mut total = header;
    for i in 0..sections {
        offsets[i] = total;
        total = add(add(total, lengths[i])?, 15)? & !15;
    }
    if total > settings.limits.max_total_bytes
        || total > u32::MAX as usize
        || texts > u32::MAX as usize
        || units > u32::MAX as usize
        || tombstones.len() > u32::MAX as usize
    {
        return Err(SearchError::ResourceLimit("ASIX file size limit"));
    }
    Ok(Layout {
        sections,
        header,
        texts,
        units,
        lengths,
        offsets,
        total,
        tomb_index: base,
    })
}

pub(crate) fn encoded_size<'a>(
    rows: impl Iterator<Item = Row<'a>>,
    tombstones: &[u32],
    settings: Encoding,
) -> Result<usize, SearchError> {
    Ok(layout(rows, tombstones, settings)?.total)
}
pub(crate) fn encode<'a>(
    rows: impl Clone + Iterator<Item = Row<'a>>,
    tombstones: &[u32],
    settings: Encoding,
) -> Result<Vec<u8>, SearchError> {
    let layout = layout(rows.clone(), tombstones, settings)?;
    let mut budget = Budget::new(settings.live_bytes, settings.limits.max_working_bytes)?;
    let mut bytes = budget.reserve(layout.total)?;
    bytes.resize(layout.total, 0);
    bytes[..4].copy_from_slice(b"ASIX");
    bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
    put(&mut bytes, 8, layout.header as u32);
    put(&mut bytes, 12, layout.sections as u32);
    bytes[16..24].copy_from_slice(&(layout.total as u64).to_le_bytes());
    bytes[32] = u8::from(settings.options.profile == Profile::AsearchTsLib);
    put(&mut bytes, 36, layout.texts as u32);
    put(&mut bytes, 40, layout.units as u32);
    put(&mut bytes, 44, tombstones.len() as u32);
    let (id, version, flags) = settings.options.normalizer.descriptor();
    bytes[48..50].copy_from_slice(&id.to_le_bytes());
    bytes[50..52].copy_from_slice(&version.to_le_bytes());
    put(&mut bytes, 52, flags);
    let mut tags = [
        *b"TEXT", *b"TLEN", *b"KEYS", *b"ORDR", *b"OLEN", *b"OTXT", *b"TOMB",
    ];
    if !tombstones.is_empty() {
        tags[layout.tomb_index] = *b"TOMB";
    }
    for (i, tag) in tags.iter().enumerate().take(layout.sections) {
        let at = 64 + i * 16;
        bytes[at..at + 4].copy_from_slice(tag);
        put(&mut bytes, at + 4, u32::from(i < 4 || tag == b"TOMB"));
        put(&mut bytes, at + 8, layout.offsets[i] as u32);
        put(&mut bytes, at + 12, layout.lengths[i] as u32);
    }
    let mut normalized_at = layout.offsets[0];
    let mut original_at = layout.offsets[5];
    for (i, row) in rows.enumerate() {
        for &unit in row.normalized {
            bytes[normalized_at..normalized_at + 2].copy_from_slice(&unit.to_le_bytes());
            normalized_at += 2;
        }
        let at = layout.offsets[1] + 2 * i;
        bytes[at..at + 2].copy_from_slice(&(row.normalized.len() as u16).to_le_bytes());
        put(&mut bytes, layout.offsets[2] + 4 * i, row.key);
        put(&mut bytes, layout.offsets[3] + 4 * i, row.order);
        if let Some(original) = row.original {
            let at = layout.offsets[4] + 2 * i;
            bytes[at..at + 2].copy_from_slice(&(original.len() as u16).to_le_bytes());
            for &unit in original {
                bytes[original_at..original_at + 2].copy_from_slice(&unit.to_le_bytes());
                original_at += 2;
            }
        }
    }
    if !tombstones.is_empty() {
        for (i, &key) in tombstones.iter().enumerate() {
            put(&mut bytes, layout.offsets[layout.tomb_index] + 4 * i, key);
        }
    }
    let hash = checksum(&bytes);
    bytes[24..32].copy_from_slice(&hash.to_le_bytes());
    Ok(bytes)
}
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
