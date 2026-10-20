use crate::Result;
use asearch_rs::{
    AsearchIndex, Doc, QueryOptions, QueryWorkspace, ResourceLimits, TransposedOptions,
};
use serde::Deserialize;
use serde_json::json;
use std::io::{BufRead, Read};

const MAX_JSONL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LINE_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputDoc {
    key: u32,
    order: Option<u32>,
    texts: Vec<String>,
}

struct UnitDoc {
    key: u32,
    order: u32,
    texts: Vec<Vec<u16>>,
}

/// Bounded local JSONL adapter. JSON parsing/UTF-8 decoding belongs to the CLI;
/// resource-bounded runtime layout construction remains in the Rust core.
pub fn build(mut reader: impl BufRead) -> Result<Vec<u8>> {
    let limits = ResourceLimits::default();
    let mut line = Vec::new();
    let mut docs = Vec::new();
    let mut total_bytes = 0;
    let mut total_texts = 0;
    let mut total_units = 0;
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_LINE_BYTES + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        total_bytes += read as u64;
        if read as u64 > MAX_LINE_BYTES || total_bytes > MAX_JSONL_BYTES {
            return Err("JSONL exceeds 1 MiB per line or 64 MiB total".into());
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let doc: InputDoc = serde_json::from_slice(&line)?;
        if doc.texts.is_empty() {
            return Err("document requires texts".into());
        }
        total_texts += doc.texts.len();
        if total_texts > limits.max_texts {
            return Err("JSONL exceeds text count limit".into());
        }
        let mut texts = Vec::new();
        texts.try_reserve_exact(doc.texts.len())?;
        for text in doc.texts {
            let length = text.encode_utf16().count();
            total_units += length;
            if length > 65535 || total_units > limits.max_total_bytes / 4 {
                return Err("JSONL exceeds UTF-16 input budget".into());
            }
            let mut units = Vec::new();
            units.try_reserve_exact(length)?;
            units.extend(text.encode_utf16());
            texts.push(units);
        }
        docs.try_reserve(1)?;
        docs.push(UnitDoc {
            key: doc.key,
            order: doc.order.unwrap_or(doc.key),
            texts,
        });
    }
    let mut refs = Vec::new();
    refs.try_reserve_exact(total_texts)?;
    refs.extend(
        docs.iter()
            .flat_map(|doc| doc.texts.iter().map(Vec::as_slice)),
    );
    let mut inputs = Vec::new();
    inputs.try_reserve_exact(docs.len())?;
    let mut start = 0;
    for doc in &docs {
        let end = start + doc.texts.len();
        inputs.push(Doc {
            key: doc.key,
            order: doc.order,
            texts: &refs[start..end],
        });
        start = end;
    }
    Ok(AsearchIndex::from_docs(&inputs, TransposedOptions::default(), limits)?.to_bytes()?)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn build_is_deterministic_and_searches_normalized_questions() {
        let a = br#"{"key":2,"texts":["ABC"]}
{"key":1,"order":5,"texts":["other","ABC"]}
"#;
        let b = br#"{"key":1,"order":5,"texts":["other","ABC"]}
{"key":2,"texts":["ABC"]}
"#;
        let bytes = build(Cursor::new(a)).unwrap();
        assert_eq!(bytes, build(Cursor::new(b)).unwrap());
        let found = search(&bytes, "ＡＢＣ").unwrap();
        assert_eq!(found["hits"][0]["key"], 2);
        assert_eq!(found["hits"][1]["key"], 1);
        assert_eq!(found["totals"], json!([2]));
    }

    #[test]
    fn jsonl_rejects_malformed_duplicate_and_oversized_documents() {
        for input in [
            r#"{"key":1,"texts":[]}"#,
            r#"{"key":-1,"texts":["a"]}"#,
            r#"{"key":1,"texts":["a"],"unexpected":true}"#,
            "{invalid}",
        ] {
            assert!(build(Cursor::new(input)).is_err());
        }
        assert!(build(Cursor::new(" ".repeat(MAX_LINE_BYTES as usize + 1))).is_err());
        assert!(build(Cursor::new("{\"key\":1,\"texts\":[\"a\"]}\n".repeat(2))).is_err());
        assert!(
            build(Cursor::new(format!(
                "{{\"key\":1,\"texts\":[\"{}\"]}}",
                "a".repeat(65536)
            )))
            .is_err()
        );
        assert!(search(b"broken", "a").is_err());
    }
}
