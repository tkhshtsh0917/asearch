//! Bounded JSONL decoding belongs to the CLI, separate from core layouts.

use crate::Result;
use asearch_engine::{Doc, ResourceLimits, TransposedOptions};
use serde::{Deserialize, de::DeserializeOwned};
use std::io::{BufRead, Read};

const MAX_JSONL_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_LINE_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InputDoc {
    pub key: u32,
    #[serde(default, deserialize_with = "optional_order")]
    pub order: Option<u32>,
    pub texts: Vec<String>,
}
// Missing means key order; explicit null is an invalid integer, just as in JS.
pub(crate) fn optional_order<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u32>, D::Error> {
    u32::deserialize(deserializer).map(Some)
}

struct UnitDoc {
    key: u32,
    order: u32,
    texts: Vec<Vec<u16>>,
}

pub(crate) fn records<T: DeserializeOwned>(
    mut reader: impl BufRead,
    mut visit: impl FnMut(T) -> Result<()>,
) -> Result<()> {
    let mut line = Vec::new();
    let mut total = 0;
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_LINE_BYTES + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if read as u64 > MAX_LINE_BYTES || total > MAX_JSONL_BYTES {
            return Err("JSONL exceeds 1 MiB per line or 64 MiB total".into());
        }
        if !line.iter().all(u8::is_ascii_whitespace) {
            visit(serde_json::from_slice(&line)?)?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct Documents {
    docs: Vec<UnitDoc>,
    texts: usize,
    units: usize,
}

impl Documents {
    pub fn push(&mut self, doc: InputDoc) -> Result<()> {
        let limits = ResourceLimits::default();
        if doc.texts.is_empty() {
            return Err("document requires texts".into());
        }
        self.texts += doc.texts.len();
        if self.texts > limits.max_texts {
            return Err("JSONL exceeds text count limit".into());
        }
        let mut texts = Vec::new();
        texts.try_reserve_exact(doc.texts.len())?;
        for text in doc.texts {
            let length = text.encode_utf16().count();
            self.units += length;
            if length > 65535 || self.units > limits.max_total_bytes / 4 {
                return Err("JSONL exceeds UTF-16 input budget".into());
            }
            let mut units = Vec::new();
            units.try_reserve_exact(length)?;
            units.extend(text.encode_utf16());
            texts.push(units);
        }
        self.docs.try_reserve(1)?;
        self.docs.push(UnitDoc {
            key: doc.key,
            order: doc.order.unwrap_or(doc.key),
            texts,
        });
        Ok(())
    }

    pub fn build(&self, tombstones: &[u32]) -> Result<Vec<u8>> {
        let mut refs = Vec::new();
        refs.try_reserve_exact(self.texts)?;
        refs.extend(
            self.docs
                .iter()
                .flat_map(|doc| doc.texts.iter().map(Vec::as_slice)),
        );
        let mut inputs = Vec::new();
        inputs.try_reserve_exact(self.docs.len())?;
        let mut start = 0;
        for doc in &self.docs {
            let end = start + doc.texts.len();
            inputs.push(Doc {
                key: doc.key,
                order: doc.order,
                texts: &refs[start..end],
            });
            start = end;
        }
        Ok(asearch_builder::build(
            &inputs,
            tombstones,
            TransposedOptions::default(),
            ResourceLimits::default(),
        )?)
    }
}
