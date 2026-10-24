//! Live-key resolution, canonical merges and server history.

use super::helpers::*;

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
