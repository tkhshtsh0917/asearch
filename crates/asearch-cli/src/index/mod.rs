mod columns;
mod delta;
pub use columns::{compact, manifest};
mod input;
pub use delta::build as delta;
#[cfg(test)]
mod tests;

use crate::Result;
use asearch_engine::{
    AsearchIndex, QueryOptions, QueryWorkspace, ResourceLimits, TransposedOptions,
};
use serde_json::json;
use std::io::{BufRead, Read};

pub fn build(reader: impl BufRead) -> Result<Vec<u8>> {
    let mut documents = input::Documents::default();
    input::records(reader, |doc| documents.push(doc))?;
    documents.build(&[])
}

pub fn search(bytes: &[u8], input: &str) -> Result<serde_json::Value> {
    let limits = ResourceLimits::default();
    let options = descriptor(bytes)?;
    let index = AsearchIndex::from_bytes(bytes, options, limits)?;
    let mut query = QueryWorkspace::new(
        QueryOptions {
            profile: options.profile,
            normalizer: options.normalizer,
            ..Default::default()
        },
        limits,
    )?;
    if input.len() > limits.max_pattern_units * 4 {
        return Err("query exceeds input limit".into());
    }
    query.update(&input.encode_utf16().collect::<Vec<_>>())?;
    let mut ws = index.workspace(&query, 7)?;
    let results = index.search(&query, &mut ws, 30, true)?;
    Ok(
        json!({"hits": results.hits.iter().map(|hit| json!({"key":hit.key,"order":hit.order,"textId":hit.text_id,"level":hit.level})).collect::<Vec<_>>(),"totals":results.totals}),
    )
}

pub fn read_index(reader: impl Read) -> Result<Vec<u8>> {
    let limit = ResourceLimits::default().max_total_bytes as u64;
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("index exceeds input byte limit".into());
    }
    Ok(bytes)
}

/// Reads the wire descriptors; the loader validates the complete segment.
fn descriptor(bytes: &[u8]) -> Result<TransposedOptions> {
    if bytes.len() < 64 {
        return Err("truncated ASIX header".into());
    }
    Ok(TransposedOptions {
        profile: match bytes[32] {
            0 => asearch_engine::Profile::Ideal,
            1 => asearch_engine::Profile::AsearchTsLib,
            _ => return Err("invalid symbol unit".into()),
        },
        normalizer: asearch_engine::Normalizer::from_descriptor(
            u16::from_le_bytes(bytes[48..50].try_into()?),
            u16::from_le_bytes(bytes[50..52].try_into()?),
            u32::from_le_bytes(bytes[52..56].try_into()?),
        )?,
        ..Default::default()
    })
}
