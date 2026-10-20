mod support;

use asearch_rs::{
    AmbigPolicy, Engine, MatchMode, Normalizer, Profile, QueryOptions, QueryWorkspace,
    ResourceLimits, SearchError, TransposedEngine, TransposedOptions,
};

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn limits() -> ResourceLimits {
    ResourceLimits {
        max_pattern_units: 256,
        ..ResourceLimits::default()
    }
}

#[test]
fn transposed_matches_cost_matrix_across_blocks_modes_and_levels() {
    let mut seed = 19_u64;
    let alphabet = ['a', 'B', '漢', '😀', ' ', 'ß'];
    let mut random = |max: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as usize % max
    };
    let mut texts = vec![
        String::new(),
        "a".repeat(62),
        "a".repeat(63),
        "a".repeat(64),
        "a".repeat(127),
        "a".repeat(200),
    ];
    for _ in 0..200 {
        let length = random(25);
        texts.push(
            (0..length)
                .map(|_| alphabet[random(alphabet.len())])
                .collect(),
        );
    }
    let mut patterns = vec![
        String::new(),
        " ".into(),
        "a B".into(),
        "😀😀".into(),
        "unknown".into(),
        "a".repeat(128),
    ];
    for _ in 0..50 {
        let length = random(16);
        patterns.push(
            (0..length)
                .map(|_| alphabet[random(alphabet.len())])
                .collect(),
        );
    }
    let corpus: Vec<_> = texts.iter().map(|text| units(text)).collect();
    let refs: Vec<_> = corpus.iter().map(Vec::as_slice).collect();
    for block_words in [1, 2, 512] {
        let engine = TransposedEngine::new(
            &refs,
            TransposedOptions {
                normalizer: Normalizer::NONE,
                block_words,
                ..TransposedOptions::default()
            },
            limits(),
        )
        .unwrap();
        for mode in [
            MatchMode::Exact,
            MatchMode::Include,
            MatchMode::WildcardSpace,
        ] {
            for k in 0..=7 {
                let mut query = QueryWorkspace::new(
                    QueryOptions {
                        normalizer: Normalizer::NONE,
                        mode,
                        wrap: false,
                        policy: AmbigPolicy::fixed(k).unwrap(),
                        ..QueryOptions::default()
                    },
                    limits(),
                )
                .unwrap();
                query.update(&units(&"a".repeat(256))).unwrap();
                let mut ws = engine.workspace(&query, 7).unwrap();
                for pattern in &patterns {
                    query.update(&units(pattern)).unwrap();
                    let results = engine.search(&query, &mut ws).unwrap();
                    for (index, text) in texts.iter().enumerate() {
                        let cost = support::oracle(pattern, text, mode);
                        let expected = (cost <= k as usize).then_some(cost as u16);
                        assert_eq!(
                            results[index], expected,
                            "B={block_words}, K={k}, mode={mode:?}, p={pattern:?}, t={text:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn dense_containers_chunk_crossings_and_fill_carries_match() {
    let texts = [
        "a".repeat(65_535),
        "ba".repeat(100),
        "a".repeat(65_535),
        String::new(),
    ];
    let data: Vec<_> = texts.iter().map(|s| units(s)).collect();
    let refs: Vec<_> = data.iter().map(Vec::as_slice).collect();
    let engine = TransposedEngine::new(&refs, TransposedOptions::default(), limits()).unwrap();
    for pattern in ["a", "a b", "b a", "😀", ""] {
        let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
        query.update(&units(pattern)).unwrap();
        let mut ws = engine.workspace(&query, 7).unwrap();
        let actual = engine.search(&query, &mut ws).unwrap();
        let wrapped = format!(" {pattern} ");
        for (i, text) in texts.iter().enumerate() {
            let cost = support::oracle(&wrapped, text, MatchMode::WildcardSpace);
            assert_eq!(
                actual[i],
                (cost <= query.max_ambig() as usize).then_some(cost as u16)
            );
        }
    }
}

#[test]
fn normalization_lone_surrogates_and_utf16_corpus_profile() {
    let corpus = [units("ｶﾞＡ😀"), vec![0xd800], units("😀")];
    let refs: Vec<_> = corpus.iter().map(Vec::as_slice).collect();
    for profile in [Profile::Ideal, Profile::AsearchTs, Profile::AsearchTsLib] {
        let engine = TransposedEngine::new(
            &refs,
            TransposedOptions {
                profile,
                ..TransposedOptions::default()
            },
            limits(),
        )
        .unwrap();
        for (input, expected) in [(units("がa😀"), 0), (vec![0xd800], 1), (units("😀"), 2)] {
            let mut query = QueryWorkspace::new(
                QueryOptions {
                    profile,
                    mode: MatchMode::Exact,
                    wrap: false,
                    policy: AmbigPolicy::fixed(0).unwrap(),
                    ..QueryOptions::default()
                },
                limits(),
            )
            .unwrap();
            query.update(&input).unwrap();
            let mut ws = engine.workspace(&query, 0).unwrap();
            let actual = engine.search(&query, &mut ws).unwrap();
            // TsLib intentionally consumes the surrogate pair as two symbols.
            let matches = !(profile == Profile::AsearchTsLib && input.contains(&0xd83d));
            assert_eq!(actual[expected], matches.then_some(0));
        }
    }
}

#[test]
fn empty_corpus_and_resource_failures_preserve_successful_query() {
    let empty = TransposedEngine::new(&[], TransposedOptions::default(), limits()).unwrap();
    let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let mut ws = empty.workspace(&query, 7).unwrap();
    assert!(empty.search(&query, &mut ws).unwrap().is_empty());
    let data = units("が");
    let corpus = [data.as_slice()];
    let constrained = ResourceLimits {
        max_pattern_units: 1,
        ..limits()
    };
    let engine = TransposedEngine::new(&corpus, TransposedOptions::default(), constrained).unwrap();
    query.update(&units("が")).unwrap();
    let mut ws = engine.workspace(&query, 7).unwrap();
    assert_eq!(engine.search(&query, &mut ws).unwrap(), &[Some(0)]);
    query.update(&units("ｶﾞ")).unwrap();
    assert_eq!(query.normalized(), units("が"));
    assert!(matches!(
        engine.search(&query, &mut ws),
        Err(SearchError::ResourceLimit(_))
    ));
    query.update(&units("が")).unwrap();
    assert_eq!(engine.search(&query, &mut ws).unwrap(), &[Some(0)]);

    assert!(
        TransposedEngine::new(
            &corpus,
            TransposedOptions {
                block_words: 0,
                ..TransposedOptions::default()
            },
            limits()
        )
        .is_err()
    );
    for constrained in [
        ResourceLimits {
            max_working_bytes: 1,
            ..limits()
        },
        ResourceLimits {
            max_slots: 64,
            ..limits()
        },
        ResourceLimits {
            max_units: 1,
            ..limits()
        },
        ResourceLimits {
            max_total_bytes: 1,
            ..limits()
        },
    ] {
        let large = units("ｶﾞa");
        assert!(
            TransposedEngine::new(
                &[large.as_slice()],
                TransposedOptions::default(),
                constrained
            )
            .is_err()
        );
    }
}
