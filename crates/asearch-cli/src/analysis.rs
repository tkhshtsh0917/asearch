//! Converts offline language resources and expands bounded server-side JSONL documents.

use crate::{Result, read_json};
use asearch_analyzer::{Analyzer, Policy, Resources};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

fn text(path: &str) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("analysis resource exceeds 32 MiB".into());
    }
    Ok(String::from_utf8(bytes)?)
}

pub fn compile(stopwords: &str, stoptags: &str, synonyms: &str) -> Result<Vec<u8>> {
    let resources = Resources::compile(&text(stopwords)?, &text(stoptags)?, &text(synonyms)?)?;
    Ok(serde_json::to_vec(&resources)?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    key: u32,
    #[serde(default, deserialize_with = "crate::index::input::optional_order")]
    order: Option<u32>,
    texts: Vec<String>,
}

pub fn analyze(dictionary: &str, resources: &str, policy: &str, input: &str) -> Result<Vec<u8>> {
    let resources: Resources = serde_json::from_value(read_json(resources)?)?;
    let policy: Policy = serde_json::from_value(read_json(policy)?)?;
    let mut analyzer = Analyzer::new(Path::new(dictionary), resources, policy)?;
    let mut keys = BTreeSet::new();
    let mut output = Vec::new();
    crate::index::input::records(BufReader::new(File::open(input)?), |doc: Document| {
        if doc.texts.len() != 1 || !keys.insert(doc.key) || keys.len() > 20000 {
            return Err("analysis documents require unique keys, one original text and at most 20000 records".into());
        }
        let mut texts = Vec::new();
        let mut seen = BTreeSet::new();
        for original in doc.texts {
            for value in analyzer.analyze(&original)? {
                if seen.insert(value.clone()) {
                    texts.push(value);
                }
            }
        }
        let row =
            serde_json::json!({"key":doc.key,"order":doc.order.unwrap_or(doc.key),"texts":texts});
        serde_json::to_writer(&mut output, &row)?;
        output.push(b'\n');
        if output.len() > 64 * 1024 * 1024 {
            return Err("analysis output exceeds 64 MiB".into());
        }
        Ok(())
    })?;
    Ok(output)
}

/// Collects a corpus vocabulary before exporting small browser query tables.
pub fn query_rules(
    dictionary: &str,
    resources: &str,
    policy: &str,
    input: &str,
) -> Result<Vec<u8>> {
    let resources: Resources = serde_json::from_value(read_json(resources)?)?;
    let policy: Policy = serde_json::from_value(read_json(policy)?)?;
    let mut analyzer = Analyzer::new(Path::new(dictionary), resources, policy)?;
    let mut originals = Vec::new();
    let mut keys = BTreeSet::new();
    crate::index::input::records(BufReader::new(File::open(input)?), |doc: Document| {
        if doc.texts.len() != 1 || !keys.insert(doc.key) || keys.len() > 20000 {
            return Err(
                "query rules require unique keys, one source text and at most 20000 records".into(),
            );
        }
        originals.extend(doc.texts);
        Ok(())
    })?;
    let rules = analyzer.query_rules(originals.iter().map(String::as_str))?;
    let bytes = serde_json::to_vec(&rules)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("query rules exceed 2 MiB".into());
    }
    Ok(bytes)
}
