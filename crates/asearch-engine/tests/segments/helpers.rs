//! Shared deterministic segment fixtures; each test owns its runtime state.

pub(super) use asearch_engine::{
    AmbigPolicy, AsearchIndex, Doc, IndexSet, MatchMode, QueryOptions, QueryWorkspace,
    ResourceLimits, TransposedOptions,
};

pub(super) fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

pub(super) fn segment(rows: &[(u32, u32, &[&str])], tombs: &[u32]) -> AsearchIndex {
    let data: Vec<Vec<Vec<u16>>> = rows
        .iter()
        .map(|(_, _, texts)| texts.iter().map(|s| units(s)).collect())
        .collect();
    let refs: Vec<Vec<&[u16]>> = data
        .iter()
        .map(|texts| texts.iter().map(Vec::as_slice).collect())
        .collect();
    let docs: Vec<_> = rows
        .iter()
        .zip(&refs)
        .map(|(&(key, order, _), texts)| Doc { key, order, texts })
        .collect();
    AsearchIndex::from_delta(
        &docs,
        tombs,
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap()
}

pub(super) fn parts() -> Vec<AsearchIndex> {
    vec![
        segment(
            &[
                (1, 2, &["abc", "abcd"]),
                (2, 1, &["old"]),
                (3, 3, &["removed"]),
            ],
            &[],
        ),
        segment(&[(2, 0, &["ｶﾞ😀", "abcd"]), (4, 7, &["abc"])], &[2]),
        segment(&[(4, 8, &["new", "abcde"])], &[3, 4]),
    ]
}

pub(super) fn expected() -> AsearchIndex {
    segment(
        &[
            (1, 2, &["abc", "abcd"]),
            (2, 0, &["ｶﾞ😀", "abcd"]),
            (4, 8, &["new", "abcde"]),
        ],
        &[],
    )
}

pub(super) fn limits() -> ResourceLimits {
    ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    }
}
