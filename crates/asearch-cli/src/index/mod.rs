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
    let index = AsearchIndex::from_bytes(bytes, TransposedOptions::default(), limits)?;
    let mut query = QueryWorkspace::new(QueryOptions::default(), limits)?;
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
