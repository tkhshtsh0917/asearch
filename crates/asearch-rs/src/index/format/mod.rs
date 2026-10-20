mod load;
mod view;
mod write;

pub(super) use load::load;
pub(super) use write::size;
pub(super) use write::write;

use crate::SearchError;
use xxhash_rust::xxh3::Xxh3;

fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = Xxh3::new();
    hash.update(&bytes[..24]);
    hash.update(&[0; 8]);
    hash.update(&bytes[32..]);
    hash.digest()
}

fn invalid() -> SearchError {
    SearchError::InvalidIndex("invalid ASIX base segment")
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(std::array::from_fn(|i| bytes[offset + i]))
}
