//! Dense/planned equivalence and positional ranking across updates.

use super::helpers::*;

#[test]
fn column_planner_matches_dense_through_edits_and_keeps_text_locations_bijective() {
    let set = IndexSet::new(parts(), limits()).unwrap();

    let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let mut dense = set.workspace(&query, 3).unwrap();
    let mut auto = set.workspace(&query, 3).unwrap();
    let before = set.available_working_bytes().unwrap();
    auto.enable_planner(&set, &query, 1024 * 1024, 4096)
        .unwrap();
    assert!(set.available_working_bytes().unwrap() < before);
    for flat in 0..set.text_count() {
        let (segment, local) = set.text_location(flat).unwrap();
        assert_eq!(set.text_id(segment, local), Some(flat));
        assert!(set.text(segment, local).is_some());
    }
    assert_eq!(set.text_location(set.text_count()), None);

    for pattern in [
        "",
        "a",
        "ab",
        "abc",
        "abcd",
        "abcde",
        "ab",
        "abx",
        "ｶ",
        "ｶﾞ",
        "ｶﾞ😀",
        "new",
        "",
    ] {
        query.update(&units(pattern)).unwrap();
        let actual = set.search(&query, &mut auto, 2, true).unwrap();
        let expected = set.search(&query, &mut dense, 2, true).unwrap();
        assert_eq!(actual.hits, expected.hits, "{pattern}");
        assert_eq!(actual.totals, expected.totals, "{pattern}");
    }
    auto.reset_incremental();
    query.update(&units("abcd")).unwrap();
    set.search(&query, &mut auto, 2, true).unwrap();
    assert!(auto.stats().all(|stats| stats.reused_rows == 0));
}

#[test]
fn positional_order_is_identical_across_segments_compaction_and_planner_edits() {
    let old = segment(
        &[(1, 0, &["xxbi"]), (2, 1, &["xbi"]), (3, 2, &["old"])],
        &[],
    );
    let new = segment(&[(3, 2, &["bi"]), (4, 3, &["yyybi", "bi alias"])], &[3]);
    let set = IndexSet::new(vec![old, new], limits()).unwrap();
    let full = segment(
        &[
            (1, 0, &["xxbi"]),
            (2, 1, &["xbi"]),
            (3, 2, &["bi"]),
            (4, 3, &["yyybi", "bi alias"]),
        ],
        &[],
    );
    let mut query = QueryWorkspace::new(
        QueryOptions {
            rank_by_position: true,
            policy: AmbigPolicy::fixed(1).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    let mut ws = set.workspace(&query, 1).unwrap();
    ws.enable_planner(&set, &query, 1024 * 1024, 0).unwrap();
    let mut dense = full.workspace(&query, 1).unwrap();
    for pattern in ["b", "bi", "bix", "bi"] {
        query.update(&units(pattern)).unwrap();
        let actual = set.search(&query, &mut ws, 2, true).unwrap();
        let expected = full.search(&query, &mut dense, 2, true).unwrap();
        assert_eq!(
            actual
                .hits
                .iter()
                .map(|h| (h.hit.key, h.hit.level, h.hit.start))
                .collect::<Vec<_>>(),
            expected
                .hits
                .iter()
                .map(|h| (h.key, h.level, h.start))
                .collect::<Vec<_>>()
        );
        assert_eq!(actual.totals, expected.totals);
    }
}
