//! Dispatches bounded CLI inputs and creates output files without replacing existing files.

#![forbid(unsafe_code)]

mod analysis;
mod bench;
mod fixtures;
mod flat;
mod index;

use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn read_json(path: &str) -> Result<serde_json::Value> {
    // CLI measurement/fixture inputs are local files, capped independently
    // from the product's future binary-index resource limits.
    const MAX_JSON: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_JSON + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON {
        return Err("JSON input exceeds 64 MiB".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_new(path: &str, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    std::io::Write::write_all(&mut file, bytes)?;
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, stopwords, stoptags, synonyms, output]
            if command == "compile-analysis-resources" =>
        {
            write_new(output, &analysis::compile(stopwords, stoptags, synonyms)?)?;
        }
        [command, dictionary, resources, policy, input, output] if command == "query-rules" => {
            write_new(
                output,
                &analysis::query_rules(dictionary, resources, policy, input)?,
            )?;
        }
        [command, dictionary, resources, policy, input, output] if command == "analyze" => {
            write_new(
                output,
                &analysis::analyze(dictionary, resources, policy, input)?,
            )?;
        }
        [command, input, output] if command == "build-flat" => {
            write_new(output, &flat::build(read_json(input)?)?)?;
        }
        [command, input] if command == "verify-fixtures" => {
            let summary = fixtures::verify(&read_json(input)?)?;
            println!("{}", serde_json::to_string(&summary)?);
        }
        [command, input, output] if command == "bench" => {
            let report = bench::measure(read_json(input)?)?;
            std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
            println!("Native paired measurements written");
        }
        [command, input, output] if command == "build" || command == "delta" => {
            let reader = BufReader::new(File::open(input)?);
            let bytes = if command == "build" {
                index::build(reader)?
            } else {
                index::delta(reader)?
            };
            write_new(output, &bytes)?;
            println!("ASIX segment written ({} bytes)", bytes.len());
        }
        [command, id, generation, output, pairs @ ..] if command == "manifest" => {
            let bytes = index::manifest(id, generation, pairs)?;
            write_new(output, &bytes)?;
            println!("Manifest written");
        }
        [command, output, inputs @ ..] if command == "compact" => {
            let paths: Vec<_> = inputs.iter().map(String::as_str).collect();
            let bytes = index::compact(&paths)?;
            write_new(output, &bytes)?;
            println!("Compacted ASIX base written ({} bytes)", bytes.len());
        }
        [command, input, query] if command == "search" => {
            let bytes = index::read_index(File::open(input)?)?;
            println!("{}", serde_json::to_string(&index::search(&bytes, query)?)?);
        }
        _ => {
            return Err(
                "Usage: asearch-cli compile-analysis-resources STOPWORDS STOPTAGS SYNONYMS NEW_RESOURCES_JSON | analyze SYSTEM_DIC RESOURCES_JSON POLICY_JSON DOCS_JSONL NEW_DOCS_JSONL | query-rules SYSTEM_DIC RESOURCES_JSON POLICY_JSON DOCS_JSONL NEW_RULES_JSON | build-flat INPUT_JSON NEW_INDEX_ASIX | verify-fixtures FIXTURE_JSON | bench INPUT_JSON OUTPUT_JSON | build DOCS_JSONL NEW_INDEX_ASIX | delta CHANGES_JSONL NEW_DELTA_ASIX | search INDEX_ASIX QUERY | compact NEW_BASE_ASIX SEGMENT_ASIX... | manifest INDEX_ID GENERATION NEW_MANIFEST_JSON PUBLIC_FILENAME=LOCAL_ASIX..."
                    .into(),
            );
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("asearch-cli: {error}");
        std::process::exit(1);
    }
}
