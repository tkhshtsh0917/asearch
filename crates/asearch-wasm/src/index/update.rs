//! Transport buffers are reserved while old-generation searches continue.
//! Finish releases transport leases under one borrow and adopts only after
//! both the core column and the new reference table have been prepared.
use super::{IndexHandle, Shared, error, expected, references::References};
use asearch_rs::{ExpectedSegment, SearchError, WorkingReservation};
use std::{cell::RefCell, mem::size_of, rc::Rc};
use wasm_bindgen::prelude::*;

struct Source {
    bytes: Vec<u8>,
    expected: Option<ExpectedSegment>,
    complete: bool,
    lease: Option<WorkingReservation>,
}
#[wasm_bindgen]
pub struct UpdateHandle {
    root: Rc<RefCell<Shared>>,
    append: bool,
    sources: Vec<Source>,
    input_bytes: usize,
    texts: usize,
    units: usize,
    _metadata: WorkingReservation,
}
impl Drop for UpdateHandle {
    fn drop(&mut self) {
        self.root.borrow_mut().busy = false;
    }
}
#[wasm_bindgen]
impl IndexHandle {
    pub fn begin_update(&self, append: bool) -> Result<UpdateHandle, JsError> {
        let result = (|| -> Result<UpdateHandle, SearchError> {
            let mut shared = self.index.borrow_mut();
            if shared.busy {
                return Err(SearchError::Busy("index update already in progress"));
            }
            let lease = shared
                .index
                .reserve_working(size_of::<UpdateHandle>() + 8 * size_of::<Source>())?;
            let mut sources = Vec::new();
            sources
                .try_reserve_exact(8)
                .map_err(|_| SearchError::ResourceLimit("update source reservation"))?;
            if sources.capacity() != 8 {
                return Err(SearchError::ResourceLimit("update metadata capacity"));
            }
            shared.busy = true;
            Ok(UpdateHandle {
                root: self.index.clone(),
                append,
                sources,
                input_bytes: 0,
                texts: 0,
                units: 0,
                _metadata: lease,
            })
        })();
        result.map_err(error)
    }
    pub fn segments_packed(&self) -> Result<Vec<u32>, JsError> {
        let result = (|| -> Result<Vec<u32>, SearchError> {
            let shared = self.index.borrow();
            let count = shared.index.segment_count() * 5;
            let _lease = shared
                .index
                .reserve_working(count * 4 + size_of::<Vec<u32>>())?;
            let mut packed = Vec::new();
            packed
                .try_reserve_exact(count)
                .map_err(|_| SearchError::ResourceLimit("segment info reservation"))?;
            if packed.capacity() != count {
                return Err(SearchError::ResourceLimit("segment info capacity"));
            }
            for part in shared.index.segments() {
                packed.extend_from_slice(&[
                    part.id as u32,
                    (part.id >> 32) as u32,
                    part.bytes as u32,
                    part.texts as u32,
                    part.dead as u32,
                ]);
            }
            Ok(packed)
        })();
        result.map_err(error)
    }
}
#[wasm_bindgen]
impl UpdateHandle {
    pub fn allocate(&mut self, header: &[u8], wire: &[u8]) -> Result<usize, JsError> {
        let result = (|| -> Result<usize, SearchError> {
            if self.sources.len() >= 8 || self.sources.last().is_some_and(|source| !source.complete)
            {
                return Err(SearchError::InvalidInput("unfinished update source"));
            }
            if header.len() != 64 || &header[..4] != b"ASIX" || header[4..6] != [1, 0] {
                return Err(SearchError::InvalidIndex("invalid streaming ASIX header"));
            }
            let length = u64::from_le_bytes(header[16..24].try_into().expect("header"));
            if !(64..=u32::MAX as u64).contains(&length) {
                return Err(SearchError::ResourceLimit("stream input length"));
            }
            let length = length as usize;
            let checked = |a: usize, b: usize| {
                a.checked_add(b)
                    .ok_or(SearchError::ResourceLimit("staging count overflow"))
            };
            let input = checked(self.input_bytes, length)?;
            let texts = checked(
                self.texts,
                u32::from_le_bytes(header[36..40].try_into().expect("header")) as usize,
            )?;
            let units = checked(
                self.units,
                u32::from_le_bytes(header[40..44].try_into().expect("header")) as usize,
            )?;
            let expected = expected::decode(wire)?;
            let shared = self.root.borrow();
            shared.index.validate_staging(
                input,
                texts,
                units,
                self.sources.len() + 1,
                self.append,
            )?;
            let lease = shared.index.reserve_working(length)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| SearchError::ResourceLimit("update transport reservation"))?;
            if bytes.capacity() != length {
                return Err(SearchError::ResourceLimit("update transport capacity"));
            }
            bytes.resize(length, 0);
            bytes[..64].copy_from_slice(header);
            let ptr = bytes.as_ptr() as usize;
            self.sources.push(Source {
                bytes,
                expected,
                complete: false,
                lease: Some(lease),
            });
            self.input_bytes = input;
            self.texts = texts;
            self.units = units;
            Ok(ptr)
        })();
        result.map_err(error)
    }
    pub fn complete_source(&mut self) -> Result<(), JsError> {
        let result = self
            .sources
            .last_mut()
            .filter(|source| !source.complete)
            .ok_or(SearchError::InvalidInput("missing update source"))
            .map(|source| source.complete = true);
        result.map_err(error)
    }
    pub fn finish(mut self) -> Result<(), JsError> {
        let result = (|| -> Result<(), SearchError> {
            if self.sources.is_empty()
                || self.sources.iter().any(|source| !source.complete)
                || (self.append && self.sources.len() != 1)
            {
                return Err(SearchError::InvalidInput("invalid complete update sources"));
            }
            let mut shared = self.root.borrow_mut();
            // Reserve the Vec<Vec<u8>> before transferring the fixed buffers.
            let outer = shared
                .index
                .reserve_working(self.sources.len() * size_of::<Vec<u8>>())?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(self.sources.len())
                .map_err(|_| SearchError::ResourceLimit("update source transfer"))?;
            if bytes.capacity() != self.sources.len() {
                return Err(SearchError::ResourceLimit(
                    "update source transfer capacity",
                ));
            }
            let mut expected = [None; 8];
            for (i, source) in self.sources.iter_mut().enumerate() {
                bytes.push(std::mem::take(&mut source.bytes));
                expected[i] = source.expected.take();
                source.lease.take();
            }
            drop(outer);
            let count = bytes.len();
            let prepared = if self.append {
                let source = bytes.pop().expect("one source");
                drop(bytes);
                shared.index.prepare_append(source, expected[0])?
            } else {
                shared.index.prepare_replace(bytes, &expected[..count])?
            };
            let refs = References::prepared(&prepared)?;
            prepared.commit();
            shared.refs = refs;
            Ok(())
        })();
        // No host Error constructor runs while the root is borrowed.
        result.map_err(error)
    }
}
