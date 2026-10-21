use asearch_engine::{
    Engine, MatchMode, MultiWord, ScalarEngine, ScalarError, ScalarLimits, ScalarWm,
};

mod support;

#[test]
fn corpus_engine_returns_complete_levels_in_input_order() {
    let texts = ["", "漢字", "漢の字😀", "😀", "ab", "aB", "x", " "];
    let limits = ScalarLimits::default();
    let engine = ScalarEngine::<MultiWord<2>>::new(&texts, limits).unwrap();
    for mode in [
        MatchMode::Exact,
        MatchMode::Include,
        MatchMode::WildcardSpace,
    ] {
        for pattern in ["", "漢 字", "😀", "ab", " "] {
            let query = ScalarWm::new(pattern, mode, limits).unwrap();
            let mut workspace = engine.workspace(&query, 3).unwrap();
            let expected: Vec<_> = texts
                .iter()
                .map(|text| {
                    let cost = support::oracle(pattern, text, mode);
                    (cost <= 3).then_some(cost as u16)
                })
                .collect();
            assert_eq!(engine.search(&query, &mut workspace).unwrap(), expected);
            assert_eq!(engine.search(&query, &mut workspace).unwrap(), expected);
        }
    }
}

#[test]
fn empty_corpus_workspace_reuse_and_aggregate_budget() {
    let limits = ScalarLimits::default();
    let query = ScalarWm::<u64>::new("a", MatchMode::Exact, limits).unwrap();
    let empty = ScalarEngine::new(&[], limits).unwrap();
    let one = ScalarEngine::new(&["a"], limits).unwrap();
    let many = ScalarEngine::new(&["a", "b"], limits).unwrap();
    let mut workspace = one.workspace(&query, 1).unwrap();
    assert_eq!(one.search(&query, &mut workspace).unwrap(), [Some(0)]);

    let longer = ScalarWm::<u64>::new("😀", MatchMode::Exact, limits).unwrap();
    let short_patterns = ScalarEngine::new(
        &["a"],
        ScalarLimits {
            max_pattern_units: 1,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        short_patterns.workspace(&longer, 1),
        Err(ScalarError::PatternLimit)
    ));
    assert_eq!(
        short_patterns.search(&longer, &mut workspace),
        Err(ScalarError::PatternLimit)
    );
    assert_eq!(
        many.search(&query, &mut workspace),
        Err(ScalarError::WorkspaceCapacity)
    );
    assert_eq!(one.search(&query, &mut workspace).unwrap(), [Some(0)]);
    assert!(empty.search(&query, &mut workspace).unwrap().is_empty());

    let tiny = ScalarEngine::new(
        &["a"],
        ScalarLimits {
            max_working_bytes: 1,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        tiny.workspace(&query, 1),
        Err(ScalarError::WorkingLimit)
    ));
    assert_eq!(
        tiny.search(&query, &mut workspace),
        Err(ScalarError::WorkingLimit)
    );
    assert_eq!(one.search(&query, &mut workspace).unwrap(), [Some(0)]);
    let mut large = one.workspace(&query, u16::MAX).unwrap();
    let strict = ScalarEngine::new(
        &["a"],
        ScalarLimits {
            max_working_bytes: 1024,
            ..limits
        },
    )
    .unwrap();
    assert_eq!(
        strict.search(&query, &mut large),
        Err(ScalarError::WorkingLimit)
    );
}
