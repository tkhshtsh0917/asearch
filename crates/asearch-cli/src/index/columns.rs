//! Server publication helpers. URLs are supplied separately from local paths;
//! no local path is copied into a public manifest.

use super::read_index;
use crate::Result;
use asearch_engine::{AsearchIndex, IndexSet, ResourceLimits, TransposedOptions};
use serde_json::{Value, json};
use sha2::{Digest, Sha384};
use std::{fs::File, io::Read};

fn load(paths: &[&str]) -> Result<(IndexSet, Vec<Value>, TransposedOptions)> {
    let limits = ResourceLimits::default();
    if !(1..=limits.max_segments).contains(&paths.len()) {
        return Err("expected 1..8 segment paths".into());
    }

    let mut sources = Vec::new();
    let mut descriptors = Vec::new();
    let mut total = 0;
    let mut options = TransposedOptions::default();

    for path in paths {
        let remaining = limits.max_total_bytes - total;
        let bytes = read_index(File::open(path)?.take(remaining as u64 + 1))?;
        total = total
            .checked_add(bytes.len())
            .ok_or("input size overflow")?;
        if total > limits.max_total_bytes || bytes.len() < 64 {
            return Err("aggregate input limit or truncated header".into());
        }
        if bytes[6..8] != [0, 0] {
            return Err("CLI publication requires ASIX minor version 0".into());
        }
        if sources.is_empty() {
            options = super::descriptor(&bytes)?;
        }
        let id = u64::from_le_bytes(bytes[24..32].try_into()?);
        let texts = u32::from_le_bytes(bytes[36..40].try_into()?);
        descriptors.push(json!({
            "id": format!("{id:016x}"),
            "bytes": bytes.len(),
            "texts": texts,
            "integrity": format!("sha384-{}", base64(&Sha384::digest(&bytes))),
        }));
        sources.push(bytes);
    }

    let empty = AsearchIndex::from_docs(&[], options, limits)?;
    let mut set = IndexSet::new(vec![empty], limits)?;
    set.replace_bytes(sources, &vec![None; paths.len()])?;
    Ok((set, descriptors, options))
}

pub fn compact(paths: &[&str]) -> Result<Vec<u8>> {
    let (set, _, _) = load(paths)?;
    Ok(set.to_bytes()?)
}

pub fn manifest(id: &str, generation: &str, pairs: &[String]) -> Result<Vec<u8>> {
    if id.is_empty()
        || id.len() > 128
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("invalid indexId".into());
    }

    let generation: u64 = generation.parse()?;
    if !(1..=9_007_199_254_740_991).contains(&generation) {
        return Err("generation must be a positive safe integer".into());
    }

    let pairs: Vec<_> = pairs
        .iter()
        .map(|pair| {
            pair.split_once('=')
                .ok_or("expected PUBLIC_FILENAME=LOCAL_ASIX")
        })
        .collect::<std::result::Result<_, _>>()?;

    // Deliberately accept portable relative publication filenames only. HTTP
    // fetching, origin policy and directory traversal are not CLI concerns.
    for (url, _) in &pairs {
        if url.is_empty()
            || url.len() > 255
            || !url.as_bytes()[0].is_ascii_alphanumeric()
            || !url
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err("public URL must be a relative filename containing ASCII letters, digits, dot, dash or underscore".into());
        }
    }

    let paths: Vec<_> = pairs.iter().map(|(_, path)| *path).collect();
    let (_set, mut segments, options) = load(&paths)?;

    let (normalizer, version, flags) = options.normalizer.descriptor();
    let unit = if options.profile == asearch_engine::Profile::AsearchTsLib {
        "utf16"
    } else {
        "code-point"
    };
    let normalizer = if normalizer == 0 { "none" } else { "ja-basic" };

    for (segment, (url, _)) in segments.iter_mut().zip(pairs) {
        segment["url"] = json!(url);
    }

    Ok(serde_json::to_vec_pretty(&json!({
        "format": "asearch-manifest/1",
        "indexId": id,
        "generation": generation,
        "index": {
            "major": 1,
            "minor": 0,
            "symbol_unit": unit,
            "normalizer": { "id": normalizer, "version": version, "options": flags },
        },
        "segments": segments,
    }))?)
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(a >> 2) as usize] as char);
        output.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests;
