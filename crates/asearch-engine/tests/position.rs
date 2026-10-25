//! Positional ranking is global, alias-aware and secondary to minimum edit distance.
use asearch_engine::{
    AmbigPolicy, AsearchIndex, Doc, MatchMode, QueryOptions, QueryWorkspace, ResourceLimits,
    TransposedOptions,
};

fn search(
    texts: &[Vec<&str>],
    pattern: &str,
    k: u8,
    positional: bool,
    limit: usize,
) -> Vec<(u32, u16, usize, usize)> {
    search_mode(
        texts,
        pattern,
        k,
        positional,
        limit,
        MatchMode::Include,
        false,
    )
}

fn search_mode(
    texts: &[Vec<&str>],
    pattern: &str,
    k: u8,
    positional: bool,
    limit: usize,
    mode: MatchMode,
    wrap: bool,
) -> Vec<(u32, u16, usize, usize)> {
    let units: Vec<Vec<Vec<u16>>> = texts
        .iter()
        .map(|texts| texts.iter().map(|s| s.encode_utf16().collect()).collect())
        .collect();
    let refs: Vec<Vec<&[u16]>> = units
        .iter()
        .map(|texts| texts.iter().map(Vec::as_slice).collect())
        .collect();
    let docs: Vec<_> = refs
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
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            mode,
            wrap,
            policy: AmbigPolicy::fixed(k).unwrap(),
            rank_by_position: positional,
            ..QueryOptions::default()
        },
        ResourceLimits::default(),
    )
    .unwrap();
    query
        .update(&pattern.encode_utf16().collect::<Vec<_>>())
        .unwrap();
    let mut ws = index.workspace(&query, k).unwrap();
    index
        .search(&query, &mut ws, limit, true)
        .unwrap()
        .hits
        .iter()
        .map(|h| (h.key, h.level, h.start, h.text_id))
        .collect()
}

#[test]
fn ranks_every_start_before_limiting_and_selects_the_best_alias() {
    let texts = vec![
        vec!["possibility"],
        vec!["xxbi"],
        vec!["xbi"],
        vec!["bi"],
        vec!["yybi", "bi alias"],
    ];
    let hits = search(&texts, "bi", 0, true, 3);
    assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), [3, 4, 2]);
    assert_eq!(hits.iter().map(|h| h.2).collect::<Vec<_>>(), [0, 0, 1]);
    assert_eq!(
        search(&texts, "bi", 0, false, 3)
            .iter()
            .map(|h| h.0)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn edit_distance_precedes_position_and_offsets_use_normalized_utf16() {
    let texts = vec![vec!["bx"], vec!["xxbi"], vec!["😀bi"], vec!["xbi"]];
    assert_eq!(
        search(&texts, "bi", 1, true, 10)
            .iter()
            .map(|h| (h.0, h.1, h.2))
            .collect::<Vec<_>>(),
        [(3, 0, 1), (1, 0, 2), (2, 0, 2), (0, 1, 0)]
    );
}

fn distance(a: &[u8], b: &[u8]) -> usize {
    let mut matrix = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in matrix.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, value) in matrix[0].iter_mut().enumerate() {
        *value = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            matrix[i][j] = (matrix[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]))
                .min(matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1);
        }
    }
    matrix[a.len()][b.len()]
}

#[test]
fn agrees_with_independent_substring_distance_oracle() {
    for pattern in ["a", "ab", "aba"] {
        let texts: Vec<String> = (0..32)
            .map(|bits| {
                (0..5)
                    .map(|j| if bits & (1 << j) == 0 { 'a' } else { 'b' })
                    .collect()
            })
            .collect();
        let input: Vec<_> = texts.iter().map(|s| vec![s.as_str()]).collect();
        let actual = search(&input, pattern, 2, true, 100);
        let mut expected: Vec<_> = texts
            .iter()
            .enumerate()
            .map(|(key, text)| {
                let (level, start) = (0..text.len())
                    .flat_map(|start| {
                        (start + 1..=text.len()).map(move |end| {
                            (
                                distance(pattern.as_bytes(), &text.as_bytes()[start..end]),
                                start,
                            )
                        })
                    })
                    .chain([(pattern.len(), usize::MAX)])
                    .min()
                    .unwrap();
                (key as u32, level as u16, start)
            })
            .filter(|h| h.1 <= 2)
            .collect();
        expected.sort_by_key(|&(key, level, start)| (level, start, key));
        assert_eq!(
            actual.iter().map(|h| (h.0, h.1, h.2)).collect::<Vec<_>>(),
            expected,
            "{pattern}"
        );
    }
}

#[test]
fn excessive_position_work_fails_before_dp_but_totals_and_stable_search_remain_available() {
    let text = "a".repeat(32768).encode_utf16().collect::<Vec<_>>();
    // Even an earlier unbeatable alias does not waive the full-work CPU guard.
    let prefix = "a".repeat(2049).encode_utf16().collect::<Vec<_>>();
    let texts = [prefix.as_slice(), text.as_slice()];
    let index = AsearchIndex::from_docs(
        &[Doc {
            key: 0,
            order: 0,
            texts: &texts,
        }],
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            rank_by_position: true,
            policy: AmbigPolicy::fixed(0).unwrap(),
            ..QueryOptions::default()
        },
        ResourceLimits {
            max_pattern_units: 4096,
            ..ResourceLimits::default()
        },
    )
    .unwrap();
    query
        .update(&"a".repeat(2049).encode_utf16().collect::<Vec<_>>())
        .unwrap();
    let mut ws = index.workspace(&query, 0).unwrap();
    assert!(matches!(
        index.search(&query, &mut ws, 1, true),
        Err(asearch_engine::SearchError::ResourceLimit(
            "positional ranking transition budget"
        ))
    ));
    assert_eq!(
        index
            .search(&query, &mut ws, 0, true)
            .unwrap()
            .totals
            .unwrap(),
        [1]
    );
    query
        .update_with_options(
            &"a".repeat(2049).encode_utf16().collect::<Vec<_>>(),
            QueryOptions {
                policy: AmbigPolicy::fixed(0).unwrap(),
                ..QueryOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        index.search(&query, &mut ws, 1, true).unwrap().hits[0].key,
        0
    );
}

// Exhaustive path enumeration is independent of the production rolling-row recurrence.
struct Paths<'a> {
    literals: Vec<u8>,
    loops: Vec<bool>,
    text: &'a [u8],
}
impl Paths<'_> {
    fn visit(&self, i: usize, j: usize, cost: usize, start: usize, best: &mut (usize, usize)) {
        if cost > best.0 {
            return;
        }
        if i == self.literals.len() && j == self.text.len() {
            *best = (*best).min((cost, start));
            return;
        }
        if i < self.literals.len() {
            self.visit(i + 1, j, cost + 1, start, best);
        }
        if j < self.text.len() {
            let touched = start.min(j);
            self.visit(i, j + 1, cost + 1, touched, best);
            if self.loops[i] {
                self.visit(i, j + 1, cost, start, best);
            }
            if i < self.literals.len() {
                self.visit(
                    i + 1,
                    j + 1,
                    cost + usize::from(self.literals[i] != self.text[j]),
                    touched,
                    best,
                );
            }
        }
    }
}

#[test]
fn positional_cost_and_start_match_all_alignment_paths_in_every_mode() {
    let samples = ["", "a", "aa", "ab", "ba", "bb", "aab", "bba", "abab"];
    let input: Vec<_> = samples.iter().map(|text| vec![*text]).collect();
    for mode in [
        MatchMode::Exact,
        MatchMode::Include,
        MatchMode::WildcardSpace,
    ] {
        for wrap in [false, true] {
            for pattern in ["", "a", "a b", " a", "b ", " "] {
                let mut literals = Vec::new();
                let mut loops =
                    vec![mode == MatchMode::Include || (mode == MatchMode::WildcardSpace && wrap)];
                for byte in pattern.bytes() {
                    if mode == MatchMode::WildcardSpace && byte == b' ' {
                        *loops.last_mut().unwrap() = true;
                    } else {
                        literals.push(byte);
                        loops.push(false);
                    }
                }
                if mode == MatchMode::Include || (mode == MatchMode::WildcardSpace && wrap) {
                    *loops.last_mut().unwrap() = true;
                }
                let mut expected = Vec::new();
                for (key, text) in samples.iter().enumerate() {
                    let mut best = (usize::MAX, usize::MAX);
                    Paths {
                        literals: literals.clone(),
                        loops: loops.clone(),
                        text: text.as_bytes(),
                    }
                    .visit(0, 0, 0, usize::MAX, &mut best);
                    if best.0 <= 2 {
                        expected.push((key as u32, best.0 as u16, best.1));
                    }
                }
                expected.sort_by_key(|&(key, cost, start)| (cost, start, key));
                let actual = search_mode(&input, pattern, 2, true, 100, mode, wrap);
                assert_eq!(
                    actual.iter().map(|h| (h.0, h.1, h.2)).collect::<Vec<_>>(),
                    expected,
                    "{pattern:?} {mode:?} wrap={wrap}"
                );
            }
        }
    }
}

#[test]
fn multi_alias_ranking_matches_independent_single_alias_searches() {
    let texts = vec![
        vec!["bx", "bi"],
        vec!["bi", "bx", "bilingual"],
        vec!["xxbi", "xbi", "bi"],
        vec!["xbi", "xbi"],
        vec!["😀bi", "xbi"],
        vec!["", "b"],
    ];
    for mode in [
        MatchMode::Exact,
        MatchMode::Include,
        MatchMode::WildcardSpace,
    ] {
        for pattern in ["bi", "b i", ""] {
            for k in [0, 1, 3, 7] {
                let mut expected = Vec::new();
                let mut first_text_id = 0;
                for (key, aliases) in texts.iter().enumerate() {
                    let best = aliases
                        .iter()
                        .enumerate()
                        .filter_map(|(offset, text)| {
                            search_mode(&[vec![text]], pattern, k, true, 1, mode, true)
                                .first()
                                .map(|&(_, level, start, _)| (level, start, first_text_id + offset))
                        })
                        .min();
                    if let Some((level, start, text_id)) = best {
                        expected.push((key as u32, level, start, text_id));
                    }
                    first_text_id += aliases.len();
                }
                expected.sort_by_key(|&(key, level, start, _)| (level, start, key));
                for limit in [0, 1, 3, texts.len()] {
                    let actual = search_mode(&texts, pattern, k, true, limit, mode, true);
                    assert_eq!(
                        actual,
                        expected[..limit.min(expected.len())],
                        "{pattern:?} {mode:?} K={k} limit={limit}"
                    );
                }
            }
        }
    }
}
