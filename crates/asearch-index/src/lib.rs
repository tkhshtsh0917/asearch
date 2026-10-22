//! ASIX v1 descriptors, validation and shared normalization.

#![forbid(unsafe_code)]
pub mod capacity;
mod limits;
mod normalize;
mod options;
pub mod view;
pub use limits::{ResourceLimits, SearchError};
pub use normalize::{NormalizeError, Normalizer};
pub use options::{MatchMode, Profile, TransposedOptions};
use xxhash_rust::xxh3::Xxh3;
/// Computes ASIX identity with the checksum field treated as zero. Input must contain the 32-byte prefix.
pub fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = Xxh3::new();
    hash.update(&bytes[..24]);
    hash.update(&[0; 8]);
    hash.update(&bytes[32..]);
    hash.digest()
}

fn invalid() -> SearchError {
    SearchError::InvalidIndex("invalid ASIX segment")
}

/// Reads a little-endian word from an already bounds-checked byte range.
pub fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

/// Reads a little-endian dword from an already bounds-checked byte range.
pub fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Reads a little-endian qword from an already bounds-checked byte range.
pub fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(std::array::from_fn(|i| bytes[offset + i]))
}

/// All texts for one stable document key. A document must contain at least one
/// text; an empty text itself is valid. Keys must be unique within a build.
#[derive(Clone, Copy, Debug)]
pub struct Doc<'a> {
    pub key: u32,
    pub order: u32,
    pub texts: &'a [&'a [u16]],
}

mod config;
pub use config::index_config;
