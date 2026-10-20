#![forbid(unsafe_code)]

mod bench;
mod fixtures;
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

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
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
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            std::io::Write::write_all(&mut file, &bytes)?;
            println!("ASIX segment written ({} bytes)", bytes.len());
        }
        [command, input, query] if command == "search" => {
            let bytes = index::read_index(File::open(input)?)?;
            println!("{}", serde_json::to_string(&index::search(&bytes, query)?)?);
        }
        _ => {
            return Err(
                "Usage: asearch-cli verify-fixtures FIXTURE_JSON | bench INPUT_JSON OUTPUT_JSON | build DOCS_JSONL NEW_INDEX_ASIX | delta CHANGES_JSONL NEW_DELTA_ASIX | search INDEX_ASIX QUERY"
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
