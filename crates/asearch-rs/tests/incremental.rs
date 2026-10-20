use asearch_rs::{
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
