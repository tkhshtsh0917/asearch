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
            mode: MatchMode::Include,
            wrap: false,
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
