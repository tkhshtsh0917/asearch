use asearch_engine::{
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

#[test]
fn atomic_append_replace_and_failures_keep_the_old_column_and_revision() {
    let mut set = IndexSet::new(
        vec![segment(&[(1, 1, &["abc"]), (3, 3, &["old"])], &[])],
        limits(),
    )
    .unwrap();
    let initial = set.to_bytes().unwrap();
    let initial_info = set.segments().collect::<Vec<_>>();
    let delta = segment(&[(1, 0, &["changed"]), (2, 2, &["new"])], &[1])
        .to_bytes()
        .unwrap();

    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let workspace = set.workspace(&query, 7).unwrap();
    assert_eq!(
        set.append_bytes(delta.clone(), None).unwrap_err().code(),
        "BUSY"
    );
    assert_eq!(set.revision(), 1);
    assert_eq!(set.to_bytes().unwrap(), initial);
    drop(workspace);
    let token = set.reserve_working(4096).unwrap();
    assert_eq!(token.capacity_bytes(), 4096);
    set.append_bytes(delta, None).unwrap();
    assert_eq!(set.revision(), 2);
    assert_eq!(set.document_count(), 3);
    let infos = set.segments().collect::<Vec<_>>();
    assert_eq!(infos[0].id, initial_info[0].id);
    assert_eq!(infos[0].dead, 1);
    let active = set.to_bytes().unwrap();
    let invalid = segment(&[(2, 8, &["conflict"])], &[]).to_bytes().unwrap();
    assert!(set.append_bytes(invalid, None).is_err());
    assert_eq!(set.revision(), 2);
    assert_eq!(set.to_bytes().unwrap(), active);
    let mut corrupt = segment(&[(8, 8, &["invalid"])], &[]).to_bytes().unwrap();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(set.replace_bytes(vec![corrupt], &[None]).is_err());
    assert_eq!(set.revision(), 2);
    assert_eq!(set.to_bytes().unwrap(), active);
    let replacement = segment(&[(9, 0, &["fresh"])], &[]).to_bytes().unwrap();
    set.replace_bytes(vec![replacement.clone()], &[None])
        .unwrap();
    assert_eq!(set.revision(), 3);
    assert_eq!(set.segment_count(), 1);
    assert_eq!(set.to_bytes().unwrap(), replacement);
    let before = set.available_working_bytes().unwrap();
    drop(token);
    assert_eq!(set.available_working_bytes().unwrap(), before + 4096);
}

#[test]
fn staging_constraints_do_not_become_permanent_runtime_limits() {
    let mut set = IndexSet::new(vec![segment(&[(1, 1, &["old"])], &[])], limits()).unwrap();
    let token = set
        .reserve_working(set.available_working_bytes().unwrap() - 512 * 1024)
        .unwrap();
    set.replace_bytes(
        vec![segment(&[(2, 2, &["new"])], &[]).to_bytes().unwrap()],
        &[None],
    )
    .unwrap();
    drop(token);

    let query = QueryWorkspace::new(QueryOptions::default(), ResourceLimits::default()).unwrap();
    assert!(set.workspace(&query, 7).is_ok());
}

#[test]
fn old_plus_new_counts_are_checked_even_for_full_replacement() {
    let small = ResourceLimits {
        max_texts: 1,
        ..limits()
    };

    let mut set = IndexSet::new(vec![segment(&[(1, 1, &["old"])], &[])], small).unwrap();
    let old = set.to_bytes().unwrap();
    let replacement = segment(&[(2, 2, &["new"])], &[]).to_bytes().unwrap();
    assert_eq!(
        set.replace_bytes(vec![replacement], &[None])
            .unwrap_err()
            .code(),
        "RESOURCE_LIMIT"
    );
    assert_eq!(set.to_bytes().unwrap(), old);
    assert_eq!(set.revision(), 1);
    // A removal-only delta adds no texts and remains possible at max_texts.
    set.append_bytes(segment(&[], &[1]).to_bytes().unwrap(), None)
        .unwrap();
    assert_eq!(set.document_count(), 0);
    assert_eq!(set.revision(), 2);
}

#[test]
fn prepared_updates_can_cancel_or_reserve_dependent_resources_before_commit() {
    let mut set = IndexSet::new(vec![segment(&[(1, 1, &["old"])], &[])], limits()).unwrap();
    let bytes = set.to_bytes().unwrap();
    let available = set.available_working_bytes().unwrap();
    let delta = segment(&[(2, 2, &["new"])], &[]).to_bytes().unwrap();
    let token = {
        let candidate = set.prepare_append(delta.clone(), None).unwrap();
        assert_eq!(candidate.text_count(), 2);
        assert_eq!(candidate.revision(), 2);
        assert!(candidate.reserve_working(usize::MAX).is_err());
        candidate.reserve_working(1024).unwrap()
    };
    assert_eq!(set.revision(), 1);
    assert_eq!(set.to_bytes().unwrap(), bytes);
    assert_eq!(set.available_working_bytes().unwrap(), available - 1024);
    drop(token);
    assert_eq!(set.available_working_bytes().unwrap(), available);
    let candidate = set.prepare_append(delta, None).unwrap();
    let token = candidate.reserve_working(1024).unwrap();
    candidate.commit();
    assert_eq!(set.revision(), 2);
    assert_eq!(set.document_count(), 2);
    let before = set.available_working_bytes().unwrap();
    drop(token);
    assert_eq!(set.available_working_bytes().unwrap(), before + 1024);
}

#[test]
fn column_planner_matches_dense_through_edits_and_keeps_text_locations_bijective() {
    let set = IndexSet::new(parts(), limits()).unwrap();

    let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let mut dense = set.workspace(&query, 3).unwrap();
    let mut auto = set.workspace(&query, 3).unwrap();
    let before = set.available_working_bytes().unwrap();
    auto.enable_planner(&set, &query, 1024 * 1024, 4096)
        .unwrap();
    assert!(set.available_working_bytes().unwrap() < before);
    for flat in 0..set.text_count() {
        let (segment, local) = set.text_location(flat).unwrap();
        assert_eq!(set.text_id(segment, local), Some(flat));
        assert!(set.text(segment, local).is_some());
    }
    assert_eq!(set.text_location(set.text_count()), None);

    for pattern in [
        "",
        "a",
        "ab",
        "abc",
        "abcd",
        "abcde",
        "ab",
        "abx",
        "ｶ",
        "ｶﾞ",
        "ｶﾞ😀",
        "new",
        "",
    ] {
        query.update(&units(pattern)).unwrap();
        let actual = set.search(&query, &mut auto, 2, true).unwrap();
        let expected = set.search(&query, &mut dense, 2, true).unwrap();
        assert_eq!(actual.hits, expected.hits, "{pattern}");
        assert_eq!(actual.totals, expected.totals, "{pattern}");
    }
    auto.reset_incremental();
    query.update(&units("abcd")).unwrap();
    set.search(&query, &mut auto, 2, true).unwrap();
    assert!(auto.stats().all(|stats| stats.reused_rows == 0));
}

#[test]
fn long_server_updates_and_compactions_match_an_independent_live_map() {
    use std::collections::BTreeMap;
    let mut live: BTreeMap<u32, String> = (0..20)
        .map(|key| (key, format!("question {key}")))
        .collect();

    fn rebuild(live: &BTreeMap<u32, String>) -> AsearchIndex {
        let units: Vec<_> = live
            .values()
            .map(|text| text.encode_utf16().collect::<Vec<_>>())
            .collect();
        let texts: Vec<_> = units.iter().map(|text| vec![text.as_slice()]).collect();
        let docs: Vec<_> = live
            .keys()
            .zip(&texts)
            .map(|(&key, texts)| Doc {
                key,
                order: key,
                texts,
            })
            .collect();
        AsearchIndex::from_docs(&docs, TransposedOptions::default(), limits()).unwrap()
    }

    let mut set = IndexSet::new(vec![rebuild(&live)], limits()).unwrap();
    for step in 0..80u32 {
        if set.segment_count() == 8 {
            let base = set.to_bytes().unwrap();
            set.replace_bytes(vec![base], &[None]).unwrap();
        }
        let key = (step * 13 + 7) % 31;
        let delta = if step % 3 == 0 {
            live.remove(&key);
            segment(&[], &[key])
        } else {
            let text = format!("changed {step} が😀 {key}");
            live.insert(key, text.clone());
            segment(&[(key, key, &[&text])], &[key])
        };
        let raw = delta.to_bytes().unwrap();
        set.append_bytes(raw, None).unwrap();
        let expected = rebuild(&live);
        assert_eq!(
            set.to_bytes().unwrap(),
            expected.to_bytes().unwrap(),
            "step {step}"
        );
        let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
        let mut ws = set.workspace(&query, 3).unwrap();
        ws.enable_planner(&set, &query, 1024 * 1024, 4096).unwrap();
        let mut full = expected.workspace(&query, 3).unwrap();
        for pattern in [
            "",
            "q",
            "qu",
            "question",
            "c",
            "ch",
            "cha",
            "changed",
            "ｶﾞ😀",
            "question",
            "changed 5",
        ] {
            query.update(&units(pattern)).unwrap();
            let actual = set.search(&query, &mut ws, 5, true).unwrap();
            let oracle = expected.search(&query, &mut full, 5, true).unwrap();
            let a: Vec<_> = actual
                .hits
                .iter()
                .map(|entry| (entry.hit.key, entry.hit.level, entry.hit.order))
                .collect();
            let b: Vec<_> = oracle
                .hits
                .iter()
                .map(|hit| (hit.key, hit.level, hit.order))
                .collect();
            assert_eq!(a, b, "step {step}, {pattern}");
            assert_eq!(actual.totals, oracle.totals, "step {step}, {pattern}");
        }
    }
}

#[test]
fn concurrent_column_planners_cannot_spend_the_same_remaining_capacity() {
    use std::sync::{Arc, Barrier};
    let text = "abcdef".repeat(50);
    let texts = [text.as_str()];
    let rows: Vec<_> = (0..200).map(|key| (key, key, texts.as_slice())).collect();
    let parts = vec![segment(&rows[..100], &[]), segment(&rows[100..], &[])];
    let set = IndexSet::new(
        parts,
        ResourceLimits {
            max_working_bytes: 2 * 1024 * 1024,
            ..limits()
        },
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let barrier = barrier.clone();
            let set = &set;
            scope.spawn(move || {
                let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
                let mut ws = set.workspace(&query, 3).unwrap();
                barrier.wait();
                ws.enable_planner(set, &query, 2 * 1024 * 1024, 4096)
                    .unwrap();
                barrier.wait();
                barrier.wait();
                query.update(&units("abc")).unwrap();
                assert!(
                    !set.search(&query, &mut ws, 3, true)
                        .unwrap()
                        .hits
                        .is_empty()
                );
            });
        }
        barrier.wait();
        barrier.wait();
        // reserve(0) checks the unsaturated sum of all live reservations.
        assert!(set.reserve_working(0).is_ok());
        barrier.wait();
    });
    assert!(set.reserve_working(0).is_ok());
}

#[test]
fn explanation_membership_checks_every_alias_and_rejects_dead_or_out_of_range_texts() {
    let set = IndexSet::new(parts(), limits()).unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            mode: MatchMode::Include,
            policy: AmbigPolicy::fixed(0).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    query.update(&[]).unwrap();

    // Segment order differs from global document order; replacement masks
    // hide the old key while keeping every alias in the replacing segment.
    for (segment, live) in [
        (0, vec![false, true, true, false]),
        (1, vec![true, true, false]),
        (2, vec![true, true]),
    ] {
        for (text, expected) in live.iter().enumerate() {
            let explanation = set.explain(segment, text, &query, None, None);
            if *expected {
                assert_eq!(
                    explanation.unwrap().unwrap().text,
                    set.original_text(segment, text).unwrap()
                );
            } else {
                assert!(explanation.is_err(), "dead text {segment}/{text}");
            }
        }
        for text in [live.len(), usize::MAX] {
            assert!(set.explain(segment, text, &query, None, None).is_err());
        }
    }
    assert!(set.explain(3, 0, &query, None, None).is_err());

    let empty = IndexSet::new(vec![segment(&[], &[])], limits()).unwrap();
    assert!(empty.explain(0, 0, &query, None, None).is_err());
}

#[test]
fn positional_order_is_identical_across_segments_compaction_and_planner_edits() {
    let old = segment(
        &[(1, 0, &["xxbi"]), (2, 1, &["xbi"]), (3, 2, &["old"])],
        &[],
    );
    let new = segment(&[(3, 2, &["bi"]), (4, 3, &["yyybi", "bi alias"])], &[3]);
    let set = IndexSet::new(vec![old, new], limits()).unwrap();
    let full = segment(
        &[
            (1, 0, &["xxbi"]),
            (2, 1, &["xbi"]),
            (3, 2, &["bi"]),
            (4, 3, &["yyybi", "bi alias"]),
        ],
        &[],
    );
    let mut query = QueryWorkspace::new(
        QueryOptions {
            rank_by_position: true,
            policy: AmbigPolicy::fixed(1).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    let mut ws = set.workspace(&query, 1).unwrap();
    ws.enable_planner(&set, &query, 1024 * 1024, 0).unwrap();
    let mut dense = full.workspace(&query, 1).unwrap();
    for pattern in ["b", "bi", "bix", "bi"] {
        query.update(&units(pattern)).unwrap();
        let actual = set.search(&query, &mut ws, 2, true).unwrap();
        let expected = full.search(&query, &mut dense, 2, true).unwrap();
        assert_eq!(
            actual
                .hits
                .iter()
                .map(|h| (h.hit.key, h.hit.level, h.hit.start))
                .collect::<Vec<_>>(),
            expected
                .hits
                .iter()
                .map(|h| (h.key, h.level, h.start))
                .collect::<Vec<_>>()
        );
        assert_eq!(actual.totals, expected.totals);
    }
}
