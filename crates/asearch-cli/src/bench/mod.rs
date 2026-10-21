mod sampling;

use crate::Result;
use asearch_engine::{Asearch, Engine, MatchMode, Profile, ScalarEngine, ScalarLimits, ScalarWm};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    texts: Vec<String>,
    patterns: Vec<String>,
    units: usize,
    seed: u64,
}

fn contribution(level: Option<u16>) -> u64 {
    level.map_or(0, |level| u64::from(level) + 1)
}

pub fn measure(value: Value) -> Result<Value> {
    let input: Input = serde_json::from_value(value)?;
    if input.texts.is_empty() || input.patterns.is_empty() || input.patterns.len() > 64 {
        return Err("benchmark needs texts and 1..64 patterns".into());
    }
    let units: usize = input
        .texts
        .iter()
        .map(|text| text.encode_utf16().count())
        .sum();
    if units != input.units || !(10_000..=10_000_000).contains(&units) {
        return Err("benchmark corpus must contain the declared 10^4..10^7 UTF-16 units".into());
    }
    let texts: Vec<_> = input.texts.iter().map(String::as_str).collect();
    let limits = ScalarLimits::default();
    let engine = ScalarEngine::<u32>::new(&texts, limits)?;
    let mut results = Vec::new();
    for pattern in &input.patterns {
        let baseline = Asearch::with_profile(pattern, MatchMode::Include, Profile::Ideal)?;
        let query = ScalarWm::new(pattern, MatchMode::Include, limits)?;
        let mut workspace = engine.workspace(&query, 3)?;

        // Compare full per-text levels before timing. Equal sums alone would
        // miss swapped/missing matches and are not correctness evidence.
        let candidate = engine.search(&query, &mut workspace)?;
        for (text, &actual) in texts.iter().zip(candidate) {
            if baseline.match_level(text, 3)? != actual {
                return Err("native benchmark semantics differ".into());
            }
        }
        let mut baseline_scan = || -> Result<u64> {
            texts.iter().try_fold(0, |sum, text| {
                Ok(sum + contribution(baseline.match_level(text, 3)?))
            })
        };
        let mut candidate_scan = || -> Result<u64> {
            Ok(engine
                .search(&query, &mut workspace)?
                .iter()
                .copied()
                .map(contribution)
                .sum())
        };
        let samples = sampling::paired(&mut baseline_scan, &mut candidate_scan, texts.len())?;
        results.push(json!({ "pattern": pattern, "measurement": samples }));
    }
    Ok(json!({
        "category": "native minimum-level corpus scan, without ranking or JS/WASM boundary",
        "baseline": "native Asearch Ideal (u32)",
        "candidate": "native ScalarEngine<u32>",
        "profile": "Ideal; Rust single-scalar casing; no normalization",
        "mode": "Include", "maxAmbig": 3,
        "units": input.units, "seed": input.seed, "texts": texts.len(),
        "timer": "std::time::Instant (milliseconds)",
        "results": results,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mismatched_corpus_metadata_and_unbounded_pattern_lists() {
        assert!(
            measure(json!({ "texts": [], "patterns": ["a"], "units": 10000, "seed": 31 })).is_err()
        );
        assert!(
            measure(json!({ "texts": ["a"], "patterns": ["a"], "units": 10000, "seed": 31 }))
                .is_err()
        );
        assert!(measure(json!({ "texts": ["a".repeat(10000)], "patterns": vec!["a"; 65], "units": 10000, "seed": 31 })).is_err());
    }
}
