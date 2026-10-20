use asearch_rs::{
    AmbigPolicy, AsearchIndex, Doc, IntentEntry, IntentIndex, MatchMode, Normalizer, Profile,
    QueryOptions, QueryWorkspace, ResourceLimits, TransposedOptions,
};
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn limits() -> ResourceLimits {
    ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    }
}
fn intent(
    expr: &[u16],
    options: TransposedOptions,
) -> Result<IntentIndex, asearch_rs::SearchError> {
    IntentIndex::from_entries(
        &[IntentEntry {
            key: 1,
            order: 1,
            expr,
        }],
        options,
        limits(),
    )
}
#[test]
fn grammar_choices_optional_escapes_and_cross_boundary_normalization_match_expansion() {
    let cases: Vec<(&str, Vec<&str>)> = vec![
        (
            "(天井)の(ライト|デンキュウ|蛍光灯)を(交換|修理)したい",
            vec![
                "天井のライトを交換したい",
                "天井のライトを修理したい",
                "天井のデンキュウを交換したい",
                "天井のデンキュウを修理したい",
                "天井の蛍光灯を交換したい",
                "天井の蛍光灯を修理したい",
            ],
        ),
        ("(ｶ|ﾊ)ﾞ", vec!["ｶﾞ", "ﾊﾞ"]),
        ("(ｶ|)ﾞ", vec!["ｶﾞ", "ﾞ"]),
        ("😀?a", vec!["a", "😀a"]),
        ("(a|b)?(c|)", vec!["", "a", "b", "c", "ac", "bc"]),
        (r"\(a\|b\)\?\*\+\\", vec!["(a|b)?*+\\"]),
        ("Ａ（Ｂ）", vec!["Ａ（Ｂ）"]),
        ("", vec![""]),
        ("()", vec![""]),
        ("a|", vec!["a", ""]),
    ];
    for profile in [Profile::Ideal, Profile::AsearchTs, Profile::AsearchTsLib] {
        for (expr, expansions) in &cases {
            let options = TransposedOptions {
                profile,
                ..TransposedOptions::default()
            };
            let index = intent(&units(expr), options).unwrap();
            let data: Vec<_> = expansions.iter().map(|s| units(s)).collect();
            let texts: Vec<_> = data.iter().map(Vec::as_slice).collect();
            let flat = AsearchIndex::from_docs(
                &[Doc {
                    key: 1,
                    order: 1,
                    texts: &texts,
                }],
                options,
                limits(),
            )
            .unwrap();
            for mode in [
                MatchMode::Exact,
                MatchMode::Include,
                MatchMode::WildcardSpace,
            ] {
                let mut query = QueryWorkspace::new(
                    QueryOptions {
                        mode,
                        profile,
                        policy: AmbigPolicy::fixed(3).unwrap(),
                        ..QueryOptions::default()
                    },
                    limits(),
                )
                .unwrap();
                let mut ws = flat.workspace(&query, 3).unwrap();
                for pattern in [
                    "",
                    "a",
                    "ac",
                    "b",
                    "c",
                    "が",
                    "ば",
                    "😀a",
                    "天井 蛍光灯 修理",
                    "(a|b)?*+\\",
                ] {
                    query.update(&units(pattern)).unwrap();
                    let actual = index.search(&query, None, 1, true).unwrap();
                    let expected = flat.search(&query, &mut ws, 1, true).unwrap();
                    assert_eq!(
                        actual.hits.first().map(|h| h.level),
                        expected.hits.first().map(|h| h.level),
                        "{expr} {profile:?} {mode:?} {pattern}"
                    );
                    assert_eq!(
                        actual
                            .totals
                            .as_ref()
                            .map(|t| &t[..=query.max_ambig() as usize]),
                        expected.totals
                    );
                    let explained = index.explain(0, &query, None).unwrap();
                    assert_eq!(
                        explained.as_ref().map(|e| e.level),
                        actual.hits.first().map(|h| h.level)
                    );
                    if let Some(explained) = explained {
                        assert!(
                            data.contains(&explained.text),
                            "{expr} -> {:?}",
                            explained.text
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn graph_matching_does_not_enumerate_exponentially_many_expansions() {
    let expr = "(a|b)".repeat(50);
    let index = intent(&units(&expr), TransposedOptions::default()).unwrap();
    let (nodes, edges) = index.graph_size();
    assert!(nodes < 2000 && edges < 3000, "{nodes} / {edges}");
    let mut query = QueryWorkspace::new(
        QueryOptions {
            mode: MatchMode::Exact,
            policy: AmbigPolicy::fixed(0).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    query.update(&units(&"a".repeat(50))).unwrap();
    assert_eq!(index.search(&query, None, 1, true).unwrap().hits.len(), 1);
    assert_eq!(
        index.explain(0, &query, None).unwrap().unwrap().text,
        units(&"a".repeat(50))
    );
}
#[test]
fn parser_and_working_limits_fail_explicitly_without_expansion_truncation() {
    for expr in ["(", ")", "?", "a??", "a*", "a+", "\\"] {
        assert!(
            matches!(
                intent(&units(expr), TransposedOptions::default()),
                Err(asearch_rs::SearchError::InvalidInput(_))
            ),
            "{expr}"
        );
    }
    let deep = format!("{}a{}", "(".repeat(65), ")".repeat(65));
    assert!(matches!(
        intent(&units(&deep), TransposedOptions::default()),
        Err(asearch_rs::SearchError::ResourceLimit(_))
    ));
    let expr = units("(a|b)");
    assert!(
        IntentIndex::from_entries(
            &[IntentEntry {
                key: 1,
                order: 1,
                expr: &expr
            }],
            TransposedOptions::default(),
            ResourceLimits {
                max_slots: 2,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        IntentIndex::from_entries(
            &[
                IntentEntry {
                    key: 1,
                    order: 1,
                    expr: &expr
                },
                IntentEntry {
                    key: 1,
                    order: 2,
                    expr: &expr
                }
            ],
            TransposedOptions::default(),
            limits()
        )
        .is_err()
    );
    let index = intent(&expr, TransposedOptions::default()).unwrap();
    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let reservation = index
        .reserve_working(index.available_working_bytes().unwrap() - 16)
        .unwrap();
    assert!(index.search(&query, None, 1, true).is_err());
    drop(reservation);
    assert!(index.search(&query, None, 1, true).is_ok());
}
#[test]
fn surrogate_pairs_across_groups_and_lone_units_follow_the_corpus_profile() {
    let expr = [
        b'(' as u16,
        0xd83d,
        b'|' as u16,
        b'a' as u16,
        b')' as u16,
        0xde00,
    ];
    for profile in [Profile::Ideal, Profile::AsearchTsLib] {
        let options = TransposedOptions {
            profile,
            normalizer: Normalizer::NONE,
            ..TransposedOptions::default()
        };
        let index = intent(&expr, options).unwrap();
        let mut query = QueryWorkspace::new(
            QueryOptions {
                profile,
                normalizer: Normalizer::NONE,
                mode: MatchMode::Exact,
                policy: AmbigPolicy::fixed(0).unwrap(),
                ..QueryOptions::default()
            },
            limits(),
        )
        .unwrap();
        query.update(&units("😀")).unwrap();
        let result = index.search(&query, None, 1, true).unwrap();
        assert_eq!(result.hits.len(), usize::from(profile == Profile::Ideal));
        if profile == Profile::Ideal {
            assert_eq!(
                index.explain(0, &query, None).unwrap().unwrap().text,
                units("😀")
            );
        }
    }
}

#[test]
fn generated_small_dags_equal_complete_expansion_with_all_normalizer_settings() {
    let alphabet = [
        b'a' as u16,
        b'B' as u16,
        0xff76,
        0xff8a,
        0xff9e,
        0xff9f,
        0xd83d,
        0xde00,
        b' ' as u16,
        0xff70,
    ];
    let mut seed = 0x7936_fea1u32;
    for flags in 0..64 {
        let normalizer = Normalizer::from_descriptor(1, 1, flags).unwrap();
        let profile = if flags % 2 == 0 {
            Profile::Ideal
        } else {
            Profile::AsearchTsLib
        };
        let options = TransposedOptions {
            normalizer,
            profile,
            ..TransposedOptions::default()
        };
        let mut expr = Vec::new();
        let mut expanded = vec![Vec::<u16>::new()];
        for part in 0..4 {
            let mut next_unit = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                alphabet[seed as usize % alphabet.len()]
            };
            let a = next_unit();
            let b = next_unit();
            expr.extend([b'(' as u16, a, b'|' as u16, b, b')' as u16]);
            let optional = (flags + part) % 3 == 0;
            if optional {
                expr.push(b'?' as u16);
            }
            let mut next = Vec::new();
            for prefix in expanded {
                for choice in [Some(a), Some(b)]
                    .into_iter()
                    .chain(optional.then_some(None))
                {
                    let mut text = prefix.clone();
                    if let Some(unit) = choice {
                        text.push(unit);
                    }
                    next.push(text);
                }
            }
            expanded = next;
        }
        let index = intent(&expr, options).unwrap();
        let refs: Vec<_> = expanded.iter().map(Vec::as_slice).collect();
        let flat = AsearchIndex::from_docs(
            &[Doc {
                key: 1,
                order: 1,
                texts: &refs,
            }],
            options,
            limits(),
        )
        .unwrap();
        let mut query = QueryWorkspace::new(
            QueryOptions {
                profile,
                normalizer,
                policy: AmbigPolicy::fixed(3).unwrap(),
                ..QueryOptions::default()
            },
            limits(),
        )
        .unwrap();
        let mut ws = flat.workspace(&query, 3).unwrap();
        for pattern in expanded.iter().take(3).cloned().chain([
            units("が"),
            units("ば"),
            units("😀"),
            units("a b"),
            vec![0xd83d],
            vec![0xde00],
        ]) {
            query.update(&pattern).unwrap();
            let actual = index.search(&query, None, 1, true).unwrap();
            let expected = flat.search(&query, &mut ws, 1, true).unwrap();
            assert_eq!(
                actual.hits.first().map(|h| h.level),
                expected.hits.first().map(|h| h.level),
                "flags {flags}, {expr:?}, {pattern:?}"
            );
            let explanation = index.explain(0, &query, None).unwrap();
            assert_eq!(
                explanation.as_ref().map(|e| e.level),
                actual.hits.first().map(|h| h.level)
            );
            if let Some(explanation) = explanation {
                assert!(expanded.contains(&explanation.text));
            }
        }
    }
}
