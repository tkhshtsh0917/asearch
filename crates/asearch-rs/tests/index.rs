mod support;

use asearch_rs::{
    AmbigPolicy, AsearchIndex, Doc, MatchMode, Normalizer, QueryOptions, QueryWorkspace,
    ResourceLimits, TransposedOptions,
};

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn limits() -> ResourceLimits {
    ResourceLimits {
        max_pattern_units: 32,
        ..ResourceLimits::default()
    }
}

#[test]
fn canonical_document_ranking_deduplication_and_exact_totals_match_oracle() {
    let data = [
        units("hello"),
        units("helo"),
        units("other"),
        units("hello"),
        units("help"),
        units("hello"),
    ];
    let first = [data[0].as_slice(), data[1].as_slice()];
    let second = [data[2].as_slice(), data[3].as_slice()];
    let third = [data[4].as_slice(), data[5].as_slice()];
    let docs = [
        Doc {
            key: 20,
            order: 3,
            texts: &first,
        },
        Doc {
            key: 10,
            order: 2,
            texts: &second,
        },
        Doc {
            key: 5,
            order: 2,
            texts: &third,
        },
    ];
    let index = AsearchIndex::from_docs(
        &docs,
        TransposedOptions {
            normalizer: Normalizer::NONE,
            ..TransposedOptions::default()
        },
        limits(),
    )
    .unwrap();
    let reversed = AsearchIndex::from_docs(
        &[docs[2], docs[1], docs[0]],
        TransposedOptions {
            normalizer: Normalizer::NONE,
            ..TransposedOptions::default()
        },
        limits(),
    )
    .unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            normalizer: Normalizer::NONE,
            mode: MatchMode::Exact,
            wrap: false,
            policy: AmbigPolicy::fixed(3).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    query.update(&units(&"a".repeat(32))).unwrap();
    let mut ws = index.workspace(&query, 7).unwrap();
    let mut other = reversed.workspace(&query, 7).unwrap();
    for pattern in ["hello", "helo", "help", "", "unknown"] {
        query.update(&units(pattern)).unwrap();
        let mut expected = Vec::new();
        let mut totals = [0; 4];
        for doc in &docs {
            let best = doc
                .texts
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    (
                        support::oracle(
                            pattern,
                            &String::from_utf16(text).unwrap(),
                            MatchMode::Exact,
                        ),
                        i,
                    )
                })
                .min()
                .unwrap();
            if best.0 <= 3 {
                totals[best.0] += 1;
                expected.push((best.0 as u16, doc.order, doc.key));
            }
        }
        expected.sort_unstable();
        for limit in [0, 1, 2, 3, 100] {
            let result = index.search(&query, &mut ws, limit, true).unwrap();
            assert_eq!(result.totals.unwrap(), totals);
            let actual: Vec<_> = result
                .hits
                .iter()
                .map(|hit| (hit.level, hit.order, hit.key))
                .collect();
            assert_eq!(
                actual,
                expected.iter().take(limit).copied().collect::<Vec<_>>()
            );
            assert_eq!(
                result.hits,
                reversed
                    .search(&query, &mut other, limit, false)
                    .unwrap()
                    .hits
            );
            assert!(
                index
                    .search(&query, &mut ws, limit, false)
                    .unwrap()
                    .totals
                    .is_none()
            );
        }
    }
    for text in 0..6 {
        assert_eq!(index.text(text), reversed.text(text));
    }
    query.update(&units("hello")).unwrap();
    let hits = index.search(&query, &mut ws, 10, true).unwrap().hits;
    assert_eq!(
        hits.iter().map(|hit| hit.text_id).collect::<Vec<_>>(),
        [1, 3, 4]
    );
}

#[test]
fn owned_texts_normalize_without_losing_originals_and_empty_indexes_work() {
    let raw = units("ｶﾞＡ😀");
    let texts = [raw.as_slice()];
    let index = AsearchIndex::from_docs(
        &[Doc {
            key: 1,
            order: 1,
            texts: &texts,
        }],
        TransposedOptions::default(),
        limits(),
    )
    .unwrap();
    assert_eq!(index.text(0).unwrap(), units("がa😀"));
    assert_eq!(index.original_text(0).unwrap(), raw);
    assert!(index.text(1).is_none());
    assert!(index.original_text(1).is_none());
    assert_eq!(index.document_count(), 1);
    assert_eq!(index.text_count(), 1);
    let empty = AsearchIndex::from_docs(&[], TransposedOptions::default(), limits()).unwrap();
    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let mut ws = empty.workspace(&query, 7).unwrap();
    let result = empty.search(&query, &mut ws, 30, true).unwrap();
    assert!(result.hits.is_empty());
    assert_eq!(result.totals.unwrap(), &[0]);
}

#[test]
fn malformed_documents_and_aggregate_workspace_budgets_are_rejected() {
    let text = units("hello");
    let texts = [text.as_slice()];
    let doc = Doc {
        key: 1,
        order: 1,
        texts: &texts,
    };
    assert!(
        AsearchIndex::from_docs(
            &[Doc { texts: &[], ..doc }],
            TransposedOptions::default(),
            limits()
        )
        .is_err()
    );
    assert!(AsearchIndex::from_docs(&[doc, doc], TransposedOptions::default(), limits()).is_err());
    let index = AsearchIndex::from_docs(&[doc], TransposedOptions::default(), limits()).unwrap();
    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let ws = index.workspace(&query, 7).unwrap();
    let required = index.capacity_bytes() + query.capacity_bytes() + ws.capacity_bytes();
    let exact = AsearchIndex::from_docs(
        &[doc],
        TransposedOptions::default(),
        ResourceLimits {
            max_working_bytes: required,
            ..limits()
        },
    )
    .unwrap();
    assert!(exact.workspace(&query, 7).is_ok());
    let short = AsearchIndex::from_docs(
        &[doc],
        TransposedOptions::default(),
        ResourceLimits {
            max_working_bytes: required - 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(short.workspace(&query, 7).is_err());
}

#[test]
fn explicit_casing_changes_candidates_and_invalid_keys_leave_stats_unchanged() {
    let data = [units("I"), units("i"), units("ı")];
    let texts: Vec<_> = data.iter().map(|text| [text.as_slice()]).collect();
    let docs: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(i, texts)| Doc {
            key: i as u32,
            order: i as u32,
            texts,
        })
        .collect();
    let index = AsearchIndex::from_docs(
        &docs,
        TransposedOptions {
            normalizer: Normalizer::NONE,
            ..TransposedOptions::default()
        },
        limits(),
    )
    .unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            normalizer: Normalizer::NONE,
            mode: MatchMode::Exact,
            wrap: false,
            policy: AmbigPolicy::fixed(0).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    query.update(&units("I")).unwrap();
    let mut ws = index.workspace(&query, 7).unwrap();
    assert_eq!(
        index
            .search(&query, &mut ws, 30, false)
            .unwrap()
            .hits
            .iter()
            .map(|hit| hit.key)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        index
            .search_with_casing(&query, &[73, 305, 73], &mut ws, 30, false)
            .unwrap()
            .hits
            .iter()
            .map(|hit| hit.key)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    let before = ws.stats();
    for keys in [&[73, 305][..], &[65, 305, 73], &[73, 0x110000, 73]] {
        assert!(
            index
                .search_with_casing(&query, keys, &mut ws, 30, false)
                .is_err()
        );
        assert_eq!(ws.stats(), before);
    }
}
