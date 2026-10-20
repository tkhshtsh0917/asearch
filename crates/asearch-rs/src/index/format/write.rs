use super::checksum;
use crate::{
    AsearchIndex, Profile, SearchError,
    capacity::{Budget, add, mul},
};

fn align(value: usize) -> Result<usize, SearchError> {
    Ok(add(value, 15)? & !15)
}

struct Layout {
    count: usize,
    header_len: usize,
    lengths: [usize; 6],
    offsets: [usize; 6],
    total: usize,
}

fn layout(index: &AsearchIndex) -> Result<Layout, SearchError> {
    let count = if index.has_original { 6 } else { 4 };
    let header_len = 64 + count * 16;
    let n = index.texts.len();
    let lengths = [
        mul(index.normalized.len(), 2)?,
        mul(n, 2)?,
        mul(n, 4)?,
        mul(n, 4)?,
        mul(n, 2)?,
        mul(index.original.len(), 2)?,
    ];
    let mut offsets = [0; 6];
    let mut total = header_len;
    for i in 0..count {
        offsets[i] = total;
        total = align(add(total, lengths[i])?)?;
    }
    if total > index.limits.max_total_bytes
        || total > u32::MAX as usize
        || n > u32::MAX as usize
        || index.normalized.len() > u32::MAX as usize
    {
        return Err(SearchError::ResourceLimit("ASIX file size limit"));
    }
    Ok(Layout {
        count,
        header_len,
        lengths,
        offsets,
        total,
    })
}

pub(in crate::index) fn size(index: &AsearchIndex) -> Result<usize, SearchError> {
    Ok(layout(index)?.total)
}

pub(in crate::index) fn write(index: &AsearchIndex) -> Result<Vec<u8>, SearchError> {
    let Layout {
        count,
        header_len,
        lengths,
        offsets,
        total,
    } = layout(index)?;
    let n = index.texts.len();
    let tags = [*b"TEXT", *b"TLEN", *b"KEYS", *b"ORDR", *b"OLEN", *b"OTXT"];
    let live = index.limits.max_working_bytes - index.available_working_bytes()?;
    let mut budget = Budget::new(live, index.limits.max_working_bytes)?;
    let mut bytes: Vec<u8> = budget.reserve(total)?;
    bytes.resize(total, 0);
    bytes[..4].copy_from_slice(b"ASIX");
    bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
    put_u32(&mut bytes, 8, header_len as u32);
    put_u32(&mut bytes, 12, count as u32);
    bytes[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    let options = index.engine.options();
    bytes[32] = u8::from(options.profile == Profile::AsearchTsLib);
    put_u32(&mut bytes, 36, n as u32);
    put_u32(&mut bytes, 40, index.normalized.len() as u32);
    let (id, version, flags) = options.normalizer.descriptor();
    bytes[48..50].copy_from_slice(&id.to_le_bytes());
    bytes[50..52].copy_from_slice(&version.to_le_bytes());
    put_u32(&mut bytes, 52, flags);
    for i in 0..count {
        let offset = 64 + i * 16;
        bytes[offset..offset + 4].copy_from_slice(&tags[i]);
        put_u32(&mut bytes, offset + 4, u32::from(i < 4));
        put_u32(&mut bytes, offset + 8, offsets[i] as u32);
        put_u32(&mut bytes, offset + 12, lengths[i] as u32);
    }
    for (i, &unit) in index.normalized.iter().enumerate() {
        bytes[offsets[0] + i * 2..offsets[0] + i * 2 + 2].copy_from_slice(&unit.to_le_bytes());
    }
    for doc in &index.documents {
        for i in doc.start..doc.end {
            bytes[offsets[1] + i * 2..offsets[1] + i * 2 + 2]
                .copy_from_slice(&(index.texts[i].normalized_len as u16).to_le_bytes());
            put_u32(&mut bytes, offsets[2] + i * 4, doc.key);
            put_u32(&mut bytes, offsets[3] + i * 4, doc.order);
            if index.has_original {
                bytes[offsets[4] + i * 2..offsets[4] + i * 2 + 2]
                    .copy_from_slice(&(index.texts[i].original_len as u16).to_le_bytes());
            }
        }
    }
    if index.has_original {
        for (i, &unit) in index.original.iter().enumerate() {
            bytes[offsets[5] + i * 2..offsets[5] + i * 2 + 2].copy_from_slice(&unit.to_le_bytes());
        }
    }
    let hash = checksum(&bytes);
    bytes[24..32].copy_from_slice(&hash.to_le_bytes());
    Ok(bytes)
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
