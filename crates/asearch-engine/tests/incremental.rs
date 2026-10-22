use asearch_engine::{
    AmbigPolicy, AsearchIndex, Doc, MatchMode, Normalizer, Profile, QueryOptions, QueryWorkspace,
    ResourceLimits, TransposedOptions,
};

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[test]
fn bounded_checkpoints_match_full_search_across_edits_modes_and_k_changes() {
    let data: Vec<_> = [
        "", "a", "abc", "xabcd", "ab c", "がく", "かき", "😀a", "😀b", "abcde", "abbd", "zzzz",
        "Iı",
    ]
    .map(units)
    .into_iter()
    .chain([vec![0xd800], vec![0xdc00], vec![0xd800, 97]])
    .chain((0..100).map(|i| units(&format!("prefix-{i}-abcdef"))))
    .collect();
    let groups: Vec<_> = data
        .chunks(2)
        .map(|chunk| chunk.iter().map(Vec::as_slice).collect::<Vec<_>>())
        .collect();
    let docs: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(i, texts)| Doc {
            key: i as u32,
            order: (i % 5) as u32,
            texts,
        })
        .collect();
    let limits = ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    };

    for profile in [Profile::Ideal, Profile::AsearchTsLib] {
        for cache_bytes in [0, 48_000, 1_000_000] {
            let index = AsearchIndex::from_docs(
                &docs,
                TransposedOptions {
                    profile,
                    block_words: 1,
                    ..TransposedOptions::default()
                },
                limits,
            )
            .unwrap();
            let mut options = QueryOptions {
                profile,
                policy: AmbigPolicy::fixed(0).unwrap(),
                ..QueryOptions::default()
            };
            let mut query = QueryWorkspace::new(options, limits).unwrap();
            let mut full = index.workspace(&query, 7).unwrap();
            let mut cached = index.workspace(&query, 7).unwrap();
            let enabled = cached
                .enable_incremental(&index, &query, cache_bytes)
                .unwrap();
            assert_eq!(enabled, cache_bytes != 0);
            let mut reused = false;
            for mode in [
                MatchMode::WildcardSpace,
                MatchMode::Include,
                MatchMode::Exact,
            ] {
                options.mode = mode;
                for wrap in [false, true] {
                    options.wrap = wrap;
                    for k in 0..=7 {
                        options.policy = AmbigPolicy::fixed(k).unwrap();
                        for pattern in [
                            "",
                            "a",
                            "ab",
                            "abc",
                            "abcd",
                            "abc",
                            "abx",
                            "abx ",
                            "abx c",
                            "ab",
                            "",
                            "ｶ",
                            "ｶﾞ",
                            "ｶﾞｸ",
                            "😀",
                            "😀a",
                            "a  b",
                            "a  bc",
                            "a b",
                            "I",
                        ] {
                            query.update_with_options(&units(pattern), options).unwrap();
                            for limit in [0, 1, 1000] {
                                let expected =
                                    index.search(&query, &mut full, limit, true).unwrap();
                                let actual =
                                    index.search(&query, &mut cached, limit, true).unwrap();
                                assert_eq!(
                                    actual.hits, expected.hits,
                                    "{profile:?} {mode:?} {wrap} {k} {pattern:?}"
                                );
                                assert_eq!(actual.totals, expected.totals);
                                reused |= cached.stats().reused_rows > 0;
                            }
                        }
                    }
                }
            }
            assert_eq!(reused, enabled);
            cached.reset_incremental();
            index.search(&query, &mut cached, 10, false).unwrap();
            assert_eq!(cached.stats().reused_rows, 0);
        }
    }
}

#[test]
fn host_casing_changes_and_lone_surrogate_composition_invalidate_checkpoints() {
    let data = [
        units("Ia"),
        units("ıa"),
        units("😀a"),
        vec![0xd83d, 97],
        vec![0xde00, 97],
    ];
    let groups: Vec<_> = data.iter().map(|text| [text.as_slice()]).collect();
    let docs: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(i, texts)| Doc {
            key: i as u32,
            order: i as u32,
            texts,
        })
        .collect();
    let limits = ResourceLimits {
        max_pattern_units: 32,
        ..ResourceLimits::default()
    };
    let index = AsearchIndex::from_docs(
        &docs,
        TransposedOptions {
            normalizer: Normalizer::NONE,
            ..TransposedOptions::default()
        },
        limits,
    )
    .unwrap();

    let mut query = QueryWorkspace::new(
        QueryOptions {
            normalizer: Normalizer::NONE,
            mode: MatchMode::Exact,
            policy: AmbigPolicy::fixed(1).unwrap(),
            ..QueryOptions::default()
        },
        limits,
    )
    .unwrap();
    let mut full = index.workspace(&query, 7).unwrap();
    let mut cached = index.workspace(&query, 7).unwrap();
    cached
        .enable_incremental(&index, &query, 1_000_000)
        .unwrap();
    for (pattern, keys) in [
        (units("Ia"), vec![73, 105, 73, 97, 97, 65]),
        (units("Ia"), vec![73, 305, 73, 97, 97, 65]),
        (vec![0xd83d], vec![0xd83d; 3]),
        (units("😀a"), vec![0x1f600, 0x1f600, 0x1f600, 97, 97, 65]),
    ] {
        query.update(&pattern).unwrap();
        let expected = index
            .search_with_casing(&query, &keys, &mut full, 10, true)
            .unwrap();
        let actual = index
            .search_with_casing(&query, &keys, &mut cached, 10, true)
            .unwrap();
        assert_eq!(actual.hits, expected.hits);
        assert_eq!(actual.totals, expected.totals);
    }
}

#[test]
fn narrowing_uses_complete_text_candidates_and_recovers_when_k_grows_or_mode_changes() {
    let mut data = vec![units("zzzz"); 1000];
    data.extend([
        units("abc"),
        units("abcdef"),
        units("abc xyz"),
        units("abcx"),
        units("かく"),
        units("がく"),
    ]);
    let groups: Vec<_> = data.iter().map(|text| [text.as_slice()]).collect();
    let docs: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(i, texts)| Doc {
            key: i as u32,
            order: i as u32,
            texts,
        })
        .collect();
    let limits = ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    };
    let index = AsearchIndex::from_docs(&docs, TransposedOptions::default(), limits).unwrap();

    let mut query = QueryWorkspace::new(QueryOptions::default(), limits).unwrap();
    let mut full = index.workspace(&query, 7).unwrap();
    let mut planned = index.workspace(&query, 7).unwrap();
    planned
        .enable_narrowing(&index, &query, usize::MAX)
        .unwrap();
    planned
        .enable_incremental(&index, &query, 2_000_000)
        .unwrap();
    let mut narrowed = 0;
    for (mode, wrap, k, pattern) in [
        (MatchMode::WildcardSpace, true, 0, "ab"),
        (MatchMode::WildcardSpace, true, 0, "abc"),
        (MatchMode::WildcardSpace, true, 0, "abcd"),
        (MatchMode::WildcardSpace, true, 1, "abcde"),
        (MatchMode::WildcardSpace, true, 1, "abcdef"),
        (MatchMode::WildcardSpace, true, 0, "abcd"),
        (MatchMode::WildcardSpace, false, 0, "abc "),
        (MatchMode::WildcardSpace, false, 0, "abc x"),
        (MatchMode::WildcardSpace, true, 0, "ｶ"),
        (MatchMode::WildcardSpace, true, 0, "ｶﾞ"),
        (MatchMode::WildcardSpace, true, 0, "ｶﾞｸ"),
        (MatchMode::Exact, true, 0, "ab"),
        (MatchMode::Exact, true, 0, "abc"),
        (MatchMode::Include, true, 0, "ab"),
        (MatchMode::Include, true, 0, "abc"),
        (MatchMode::Include, true, 0, "unknown"),
        (MatchMode::Include, true, 0, "unknown x"),
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
        let expected = index.search(&query, &mut full, 1, true).unwrap();
        let actual = index.search(&query, &mut planned, 1, true).unwrap();
        assert_eq!(actual.hits, expected.hits, "{pattern} {k} {mode:?}");
        assert_eq!(actual.totals, expected.totals);
        narrowed += usize::from(planned.stats().narrowed_texts.is_some());
        if mode == MatchMode::Exact || pattern == "ｶﾞ" || pattern == "abcde" {
            assert!(planned.stats().narrowed_texts.is_none());
        }
    }
    assert!(narrowed > 3);
}
