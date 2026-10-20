use crate::Result;
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

fn batch(operation: &mut impl FnMut() -> Result<u64>, repeats: usize) -> Result<(f64, u64)> {
    let mut checksum = 0;
    let start = Instant::now();
    for _ in 0..repeats {
        checksum += black_box(operation()?);
    }
    Ok((start.elapsed().as_secs_f64() * 1000.0, checksum))
}

pub fn paired(
    baseline: &mut impl FnMut() -> Result<u64>,
    candidate: &mut impl FnMut() -> Result<u64>,
    texts: usize,
) -> Result<Value> {
    for _ in 0..10 {
        if black_box(baseline()?) != black_box(candidate()?) {
            return Err("native benchmark warmup differs".into());
        }
    }
    let mut repeats = 1;
    while repeats < 1024 {
        let a = batch(baseline, repeats)?;
        let b = batch(candidate, repeats)?;
        if a.1 != b.1 {
            return Err("native calibration differs".into());
        }
        if a.0.min(b.0) >= 4.0 {
            break;
        }
        repeats *= 2;
    }

    let mut runs = Vec::new();
    let mut checksum = 0u64;
    for run in 0..5 {
        let mut baseline_samples = Vec::with_capacity(30);
        let mut candidate_samples = Vec::with_capacity(30);
        for sample in 0..30 {
            let (a, b) = if (run + sample) % 2 == 0 {
                (batch(baseline, repeats)?, batch(candidate, repeats)?)
            } else {
                let b = batch(candidate, repeats)?;
                (batch(baseline, repeats)?, b)
            };
            if a.1 != b.1 || a.0 <= 0.0 || b.0 <= 0.0 {
                return Err("invalid native timing sample or checksum mismatch".into());
            }
            checksum += a.1 + b.1;
            let operations = texts as f64 * repeats as f64;
            baseline_samples.push(a.0 / operations);
            candidate_samples.push(b.0 / operations);
        }
        runs.push(json!({ "baseline": baseline_samples, "candidate": candidate_samples }));
    }
    Ok(json!({ "repeats": repeats, "checksum": checksum, "runs": runs }))
}
