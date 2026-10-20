use asearch_rs::{
    AmbigPolicy, AsearchIndex, CoordinateSpace, Doc, IndexSet, MatchMode, Profile, QueryOptions,
    QueryWorkspace, ResourceLimits, TransposedOptions,
};
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn set(text: &str, options: TransposedOptions) -> IndexSet {
    let input = units(text);
    let texts = [input.as_slice()];
    let doc = Doc {
        key: 1,
        order: 1,
        texts: &texts,
    };
    IndexSet::new(
        vec![AsearchIndex::from_docs(&[doc], options, limits()).unwrap()],
        limits(),
    )
    .unwrap()
}
fn limits() -> ResourceLimits {
    ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    }
}
fn query(pattern: &str, mode: MatchMode, profile: Profile) -> QueryWorkspace {
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
    query.update(&units(pattern)).unwrap();
    query
}
#[test]
fn original_offsets_and_shortest_leftmost_alignment_follow_normalization() {
    for (text, pattern, mode, expected) in [
        ("ｶﾞｸ", "が", MatchMode::Include, vec![(0, 2)]),
        ("😀a", "a", MatchMode::Include, vec![(2, 3)]),
        ("abxxab", "ab", MatchMode::Include, vec![(0, 2)]),
        (
            "a x a b",
            "a b",
            MatchMode::WildcardSpace,
            vec![(4, 5), (6, 7)],
        ),
        ("axb", "ab", MatchMode::Exact, vec![(0, 3)]),
        ("ＡＢＣ", "abc", MatchMode::Exact, vec![(0, 3)]),
        ("anything", "", MatchMode::Include, vec![]),
    ] {
        let index = set(text, TransposedOptions::default());
        let query = query(pattern, mode, Profile::Ideal);
        let explanation = index.explain(0, 0, &query, None, None).unwrap().unwrap();
        assert_eq!(explanation.text, units(text));
        assert_eq!(explanation.coordinate_space, CoordinateSpace::OriginalUtf16);
        assert_eq!(explanation.spans, expected, "{text} / {pattern}");
    }
}
#[test]
fn explanation_levels_equal_bulk_for_all_profiles_modes_and_small_strings() {
    let mut texts = vec![String::new()];
    for _ in 0..4 {
        let previous = texts.clone();
        for text in previous {
            for symbol in ['a', 'b', ' '] {
                let next = format!("{text}{symbol}");
                if !texts.contains(&next) {
                    texts.push(next);
                }
            }
        }
    }
    for profile in [Profile::Ideal, Profile::AsearchTs, Profile::AsearchTsLib] {
        for text in &texts {
            let options = TransposedOptions {
                profile,
                ..TransposedOptions::default()
            };
            let index = set(text, options);
            for mode in [
                MatchMode::Exact,
                MatchMode::Include,
                MatchMode::WildcardSpace,
            ] {
                let mut query = query("", mode, profile);
                let mut ws = index.workspace(&query, 3).unwrap();
                for pattern in ["", "a", "b", "ab", "a b", " a ", "b a", "😀", "aaa"] {
                    query.update(&units(pattern)).unwrap();
                    let result = index.search(&query, &mut ws, 1, false).unwrap();
                    let explanation = index.explain(0, 0, &query, None, None).unwrap();
                    assert_eq!(
                        explanation.as_ref().map(|e| e.level),
                        result.hits.first().map(|e| e.hit.level),
                        "{profile:?} / {mode:?} / {text:?} / {pattern:?}"
                    );
                    if let Some(explanation) = explanation {
                        assert!(
                            explanation
                                .spans
                                .windows(2)
                                .all(|pair| pair[0].1 < pair[1].0)
                        );
                        assert!(
                            explanation
                                .spans
                                .iter()
                                .all(|&(a, b)| a < b && b <= units(text).len())
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn supplied_originals_refs_and_explanation_capacity_are_validated() {
    let index = set("ｶﾞ", TransposedOptions::default());
    let query = query("が", MatchMode::Exact, Profile::Ideal);
    let explanation = index
        .explain(0, 0, &query, None, Some(&units("ガ")))
        .unwrap()
        .unwrap();
    assert_eq!(explanation.text, units("ガ"));
    assert_eq!(explanation.spans, vec![(0, 1)]);
    assert!(
        index
            .explain(0, 0, &query, None, Some(&units("different")))
            .is_err()
    );
    assert!(index.explain(1, 0, &query, None, None).is_err());
    assert!(index.explain(0, 1, &query, None, None).is_err());
    let remaining = index.available_working_bytes().unwrap();
    let reserved = index.reserve_working(remaining - 32).unwrap();
    assert!(matches!(
        index.explain(0, 0, &query, None, None),
        Err(asearch_rs::SearchError::ResourceLimit(_))
    ));
    drop(reserved);
    assert!(index.explain(0, 0, &query, None, None).unwrap().is_some());
}

#[test]
fn tiny_alignment_paths_independently_verify_endpoint_and_start_ties() {
    #[allow(clippy::too_many_arguments)]
    fn paths(
        pattern: &[u8],
        text: &[u8],
        loops: &[bool],
        i: usize,
        j: usize,
        cost: u16,
        start: Option<usize>,
        end: usize,
        best: &mut Option<(u16, usize, std::cmp::Reverse<usize>)>,
    ) {
        if cost > 3 {
            return;
        }
        if i == pattern.len() && j == text.len() {
            let score = (cost, end, std::cmp::Reverse(start.unwrap_or(end)));
            if best.is_none_or(|old| score < old) {
                *best = Some(score);
            }
            return;
        }
        if i < pattern.len() {
            paths(pattern, text, loops, i + 1, j, cost + 1, start, end, best);
        }
        if j < text.len() {
            paths(
                pattern,
                text,
                loops,
                i,
                j + 1,
                cost + 1,
                start.or(Some(j)),
                j + 1,
                best,
            );
            if loops[i] {
                paths(pattern, text, loops, i, j + 1, cost, start, end, best);
            }
            if i < pattern.len() {
                paths(
                    pattern,
                    text,
                    loops,
                    i + 1,
                    j + 1,
                    cost + u16::from(pattern[i] != text[j]),
                    start.or(Some(j)),
                    j + 1,
                    best,
                );
            }
        }
    }
    let mut texts = vec![String::new()];
    for _ in 0..3 {
        let previous = texts.clone();
        for text in previous {
            for symbol in ['a', 'b', ' '] {
                let next = format!("{text}{symbol}");
                if !texts.contains(&next) {
                    texts.push(next);
                }
            }
        }
    }
    for text in &texts {
        let index = set(text, TransposedOptions::default());
        for mode in [
            MatchMode::Exact,
            MatchMode::Include,
            MatchMode::WildcardSpace,
        ] {
            for pattern in ["", "a", "ab", "a b", "b a", " a "] {
                let query = query(pattern, mode, Profile::Ideal);
                let bytes: Vec<_> = query.pattern().iter().map(|&unit| unit as u8).collect();
                let mut literals = Vec::new();
                let mut loops = vec![mode == MatchMode::Include];
                for symbol in bytes {
                    if mode == MatchMode::WildcardSpace && symbol == b' ' {
                        *loops.last_mut().unwrap() = true;
                    } else {
                        literals.push(symbol);
                        loops.push(false);
                    }
                }
                if mode == MatchMode::Include {
                    *loops.last_mut().unwrap() = true;
                }
                let mut best = None;
                paths(
                    &literals,
                    text.as_bytes(),
                    &loops,
                    0,
                    0,
                    0,
                    None,
                    0,
                    &mut best,
                );
                let actual = index.explain(0, 0, &query, None, None).unwrap();
                let actual_score = actual.as_ref().map(|explanation| {
                    (
                        explanation.level,
                        explanation.spans.last().map_or(0, |span| span.1),
                        std::cmp::Reverse(explanation.spans.first().map_or(0, |span| span.0)),
                    )
                });
                assert_eq!(actual_score, best, "{mode:?} {pattern:?} {text:?}");
            }
        }
    }
}

#[test]
fn absent_originals_use_explicit_normalized_coordinates_and_accept_valid_override() {
    let original = set("ｶﾞ😀", TransposedOptions::default());
    let mut bytes = original.to_bytes().unwrap();
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = 64 + 16 * i;
        if &bytes[at..at + 4] == b"OLEN" {
            bytes[at..at + 4].copy_from_slice(b"XLEN");
        }
        if &bytes[at..at + 4] == b"OTXT" {
            bytes[at..at + 4].copy_from_slice(b"XTXT");
        }
    }
    bytes[24..32].fill(0);
    let hash = xxhash_rust::xxh3::xxh3_64(&bytes);
    bytes[24..32].copy_from_slice(&hash.to_le_bytes());
    let part =
        AsearchIndex::from_owned_bytes(bytes, TransposedOptions::default(), limits()).unwrap();
    let index = IndexSet::new(vec![part], limits()).unwrap();
    let query = query("が😀", MatchMode::Exact, Profile::Ideal);
    let explanation = index.explain(0, 0, &query, None, None).unwrap().unwrap();
    assert_eq!(
        explanation.coordinate_space,
        CoordinateSpace::NormalizedUtf16
    );
    assert_eq!(explanation.text, units("が😀"));
    assert_eq!(explanation.spans, vec![(0, 3)]);
    let explanation = index
        .explain(0, 0, &query, None, Some(&units("ｶﾞ😀")))
        .unwrap()
        .unwrap();
    assert_eq!(explanation.coordinate_space, CoordinateSpace::OriginalUtf16);
    assert_eq!(explanation.spans, vec![(0, 4)]);
}
