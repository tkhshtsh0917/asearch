//! Trusted-manifest checks happen before allocating candidate layouts. A
//! checksum protects corruption; SHA-384 binds bytes to the trusted descriptor.

use crate::SearchError;
use sha2::{Digest, Sha384};

#[derive(Clone, Copy, Debug)]
pub struct ExpectedSegment {
    pub id: u64,
    pub bytes: usize,
    pub texts: usize,
    pub sha384: [u8; 48],
}

impl ExpectedSegment {
    pub fn verify(&self, bytes: &[u8]) -> Result<(), SearchError> {
        if bytes.len() < 64 || bytes.len() != self.bytes {
            return Err(SearchError::InvalidIndex("manifest byte count mismatch"));
        }
        let id = u64::from_le_bytes(std::array::from_fn(|i| bytes[24 + i]));
        let texts = u32::from_le_bytes(std::array::from_fn(|i| bytes[36 + i]));
        if id != self.id || texts as usize != self.texts {
            return Err(SearchError::InvalidIndex(
                "manifest content descriptor mismatch",
            ));
        }
        if Sha384::digest(bytes).as_slice() != self.sha384 {
            return Err(SearchError::InvalidIndex("manifest SHA-384 mismatch"));
        }
        Ok(())
    }
}
