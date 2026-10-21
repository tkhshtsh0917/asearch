#![no_main]
use asearch_engine::{
    AmbigPolicy, IntentEntry, IntentIndex, Profile, QueryOptions, QueryWorkspace, ResourceLimits,
    SearchError, TransposedOptions,
};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if data.len() > 512 {
        return;
    }
    let units: Vec<_> = data
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let limits = ResourceLimits {
        max_total_bytes: 4096,
        max_units: 1024,
        max_texts: 8,
        max_slots: 4096,
        max_working_bytes: 2 * 1024 * 1024,
        max_pattern_units: 16,
        ..ResourceLimits::default()
    };
    let profile = if data.first().is_some_and(|byte| byte & 1 != 0) {
        Profile::AsearchTsLib
    } else {
        Profile::Ideal
    };
    let options = TransposedOptions {
        profile,
        ..TransposedOptions::default()
    };
    let Ok(index) = IntentIndex::from_entries(
        &[IntentEntry {
            key: 1,
            order: 1,
            expr: &units,
        }],
        options,
        limits,
    ) else {
        return;
    };
    let mut query = QueryWorkspace::new(
        QueryOptions {
            profile,
            policy: AmbigPolicy::fixed(3).unwrap(),
            ..QueryOptions::default()
        },
        limits,
    )
    .unwrap();
    if query.update(&units[..units.len().min(16)]).is_err() {
        return;
    }
    if let Ok(result) = index.search(&query, None, 1, true) {
        if let Some(hit) = result.hits.first() {
            match index.explain(hit.entry, &query, None) {
                Ok(Some(explanation)) => assert_eq!(explanation.level, hit.level),
                Err(SearchError::ResourceLimit(_)) => (),
                other => panic!("matched intent must have an explanation: {other:?}"),
            }
        }
    }
});
