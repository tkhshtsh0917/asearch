use super::{IndexHandle, config::index_config, error};
use asearch_rs::{ResourceLimits, SearchError, TransposedOptions};
use wasm_bindgen::prelude::*;

/// A fixed allocation. JS refreshes its memory view after every await and
/// writes chunks directly into this buffer; Rust only reads it at finish.
#[wasm_bindgen]
pub struct StreamHandle {
    bytes: Vec<u8>,
    options: TransposedOptions,
    limits: ResourceLimits,
}

#[wasm_bindgen]
impl StreamHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(header: &[u8], config: &[u32]) -> Result<StreamHandle, JsError> {
        let (options, limits) = index_config(config).map_err(error)?;
        if header.len() != 64 || &header[..4] != b"ASIX" || header[4..6] != [1, 0] {
            return Err(error(SearchError::InvalidIndex(
                "invalid streaming ASIX header",
            )));
        }
        let declared = u64::from_le_bytes(std::array::from_fn(|i| header[16 + i]));
        let length = usize::try_from(declared).map_err(|_| {
            error(SearchError::ResourceLimit(
                "stream length cannot fit address space",
            ))
        })?;
        if length < 64
            || length > limits.max_total_bytes
            || length
                .checked_add(std::mem::size_of::<Self>())
                .is_none_or(|n| n > limits.max_working_bytes)
            || length > u32::MAX as usize
        {
            return Err(error(SearchError::ResourceLimit(
                "stream input capacity limit",
            )));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| error(SearchError::ResourceLimit("stream reservation failed")))?;
        if bytes.capacity() > limits.max_total_bytes
            || bytes
                .capacity()
                .checked_add(std::mem::size_of::<Self>())
                .is_none_or(|n| n > limits.max_working_bytes)
        {
            return Err(error(SearchError::ResourceLimit(
                "stream input capacity limit",
            )));
        }
        bytes.resize(length, 0);
        bytes[..64].copy_from_slice(header);
        Ok(Self {
            bytes,
            options,
            limits,
        })
    }

    pub fn input_ptr(&self) -> usize {
        self.bytes.as_ptr() as usize
    }

    pub fn finish(self, expected: &[u8]) -> Result<IndexHandle, JsError> {
        let Self {
            bytes,
            options,
            limits,
        } = self;
        IndexHandle::wrap(
            super::expected::load(bytes, options, limits, expected).map_err(error)?,
            options,
            limits,
        )
    }
}

#[wasm_bindgen]
pub fn index_memory() -> JsValue {
    wasm_bindgen::memory()
}
