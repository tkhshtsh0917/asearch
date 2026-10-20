use asearch_rs::{
    AmbigPolicy, AsearchIndex, Doc, IndexSet, MatchMode, QueryOptions, QueryWorkspace,
    ResourceLimits, TransposedOptions,
};
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn segment(rows: &[(u32, u32, &[&str])], tombs: &[u32]) -> AsearchIndex {
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
fn parts() -> Vec<AsearchIndex> {
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
fn expected() -> AsearchIndex {
    segment(
        &[
            (1, 2, &["abc", "abcd"]),
            (2, 0, &["ｶﾞ😀", "abcd"]),
            (4, 8, &["new", "abcde"]),
        ],
        &[],
    )
}
fn limits() -> ResourceLimits {
    ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    }
}

#[test]
fn segment_merges_apply_tombs_before_top_n_and_totals_and_compact_canonically() {
    let set = IndexSet::new(parts(), limits()).unwrap();
    let full = expected();
    assert_eq!(set.segment_count(), 3);
    assert_eq!(set.document_count(), 3);
    assert_eq!(set.to_bytes().unwrap(), full.to_bytes().unwrap());
    let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let mut ws = set.workspace(&query, 7).unwrap();
    let mut scalar = full.workspace(&query, 7).unwrap();
    for mode in [
        MatchMode::Exact,
        MatchMode::Include,
        MatchMode::WildcardSpace,
    ] {
        for wrap in [false, true] {
            for k in 0..=7 {
                for pattern in [
                    "",
                    "a",
                    "abc",
                    "abcd",
                    "new",
                    "old",
                    "removed",
                    "ｶﾞ😀",
                    "none",
                ] {
                    query
                        .update_with_options(
                            &units(pattern),
                            QueryOptions {
                                mode,
                                wrap,
                                policy: AmbigPolicy::fixed(k).unwrap(),
                                ..QueryOptions::default()
                            },
                        )
                        .unwrap();
                    for limit in [0, 1, 2, 100] {
                        let expected = full.search(&query, &mut scalar, limit, true).unwrap();
                        let actual = set.search(&query, &mut ws, limit, true).unwrap();
                        assert_eq!(
                            actual
                                .hits
                                .iter()
                                .map(|r| (r.hit.key, r.hit.order, r.hit.level))
                                .collect::<Vec<_>>(),
                            expected
                                .hits
                                .iter()
                                .map(|r| (r.key, r.order, r.level))
                                .collect::<Vec<_>>(),
                            "{pattern} {mode:?} {k}"
                        );
                        assert_eq!(actual.totals, expected.totals);
                        for hit in actual.hits {
                            assert_eq!(
                                set.text(hit.segment, hit.hit.text_id),
                                full.text(
                                    expected
                                        .hits
                                        .iter()
                                        .find(|r| r.key == hit.hit.key)
                                        .unwrap()
                                        .text_id
                                )
                            );
                        }
                    }
                }
            }
        }
    }
    assert_eq!(ws.stats().count(), 3);
    assert_eq!(set.original_text(1, 0), Some(units("ｶﾞ😀").as_slice()));
}
#[test]
fn duplicate_live_keys_and_aggregate_resources_fail_before_search() {
    assert!(IndexSet::new(vec![], limits()).is_err());
    assert!(
        IndexSet::new(
            vec![
                segment(&[(1, 1, &["a"])], &[]),
                segment(&[(1, 1, &["b"])], &[])
            ],
            limits()
        )
        .is_err()
    );
    assert!(
        IndexSet::new(
            parts(),
            ResourceLimits {
                max_segments: 2,
                ..limits()
            }
        )
        .is_err()
    );
    // All retained texts count, including texts hidden by a later tombstone.
    assert!(
        IndexSet::new(
            parts(),
            ResourceLimits {
                max_texts: 6,
                ..limits()
            }
        )
        .is_err()
    );
    let set = IndexSet::new(parts(), limits()).unwrap();
    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let workspace = set.workspace(&query, 7).unwrap();
    let required = limits().max_working_bytes - set.available_working_bytes().unwrap();
    drop(workspace);
    let exact = IndexSet::new(
        parts(),
        ResourceLimits {
            max_working_bytes: required,
            ..limits()
        },
    )
    .unwrap();
    let ws = exact.workspace(&query, 7).unwrap();
    assert!(exact.workspace(&query, 7).is_err());
    assert!(exact.to_bytes().is_err());
    drop(ws);
    assert!(exact.workspace(&query, 7).is_ok());
    assert!(exact.to_bytes().is_ok());
    let short = IndexSet::new(
        parts(),
        ResourceLimits {
            max_working_bytes: required - 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(short.workspace(&query, 7).is_err());
}

#[test]
fn one_to_eight_segment_cursors_return_distinct_ranked_keys_and_exact_totals() {
    let mut query = QueryWorkspace::new(
        QueryOptions {
            policy: AmbigPolicy::fixed(0).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    query.update(&units("same")).unwrap();
    for n in 1..=8 {
        let pieces = (0..n)
            .map(|i| segment(&[(i, i % 3, &["same"])], &[]))
            .collect();
        let set = IndexSet::new(pieces, limits()).unwrap();
        let mut ws = set.workspace(&query, 0).unwrap();
        let mut expected: Vec<_> = (0..n).collect();
        expected.sort_by_key(|&i| (i % 3, i));
        for limit in [0, 1, 8] {
            let result = set.search(&query, &mut ws, limit, true).unwrap();
            assert_eq!(result.totals, Some(&[n as usize][..]));
            assert_eq!(
                result.hits.iter().map(|h| h.hit.key).collect::<Vec<_>>(),
                expected.iter().take(limit).copied().collect::<Vec<_>>()
            );
        }
    }
}

fn without_originals(index: AsearchIndex) -> AsearchIndex {
    use xxhash_rust::xxh3::Xxh3;
    let mut bytes = index.to_bytes().unwrap();
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = 64 + 16 * i;
        if &bytes[at..at + 4] == b"OLEN" {
            bytes[at..at + 4].copy_from_slice(b"ZLEN");
        } else if &bytes[at..at + 4] == b"OTXT" {
            bytes[at..at + 4].copy_from_slice(b"ZTXT");
        }
    }
    let mut h = Xxh3::new();
    h.update(&bytes[..24]);
    h.update(&[0; 8]);
    h.update(&bytes[32..]);
    bytes[24..32].copy_from_slice(&h.digest().to_le_bytes());
    AsearchIndex::from_bytes(
        &bytes,
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap()
}
#[test]
fn compaction_never_fabricates_or_discards_live_originals() {
    assert!(
        IndexSet::new(
            vec![
                segment(&[(1, 1, &["Ａ"])], &[]),
                without_originals(segment(&[(2, 2, &["Ｂ"])], &[]))
            ],
            limits()
        )
        .is_err()
    );
    let set = IndexSet::new(
        vec![
            segment(&[(1, 1, &["Ａ"])], &[]),
            without_originals(segment(&[(2, 2, &["Ｂ"])], &[1])),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(
        set.to_bytes().unwrap(),
        without_originals(segment(&[(2, 2, &["Ｂ"])], &[]))
            .to_bytes()
            .unwrap()
    );
    let removed = IndexSet::new(
        vec![
            without_originals(segment(&[(1, 1, &["Ａ"])], &[])),
            segment(&[], &[1]),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(removed.document_count(), 0);
    assert_eq!(
        removed.to_bytes().unwrap(),
        segment(&[], &[]).to_bytes().unwrap()
    );
}
