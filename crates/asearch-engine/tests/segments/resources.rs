//! Aggregate reservations across segments and sessions.

use super::helpers::*;

#[test]
fn duplicate_live_keys_and_aggregate_resources_fail_before_search() {
    assert!(IndexSet::new(vec![], limits()).is_err());
    assert!(
        IndexSet::new(
            vec![
                segment(&[(1, 1, &["a"])], &[]),
                segment(&[(1, 1, &["b"])], &[])
            ],
            limits()
        )
        .is_err()
    );
    assert!(
        IndexSet::new(
            parts(),
            ResourceLimits {
                max_segments: 2,
                ..limits()
            }
        )
        .is_err()
    );
    // All retained texts count, including texts hidden by a later tombstone.
    assert!(
        IndexSet::new(
            parts(),
            ResourceLimits {
                max_texts: 6,
                ..limits()
            }
        )
        .is_err()
    );
    let set = IndexSet::new(parts(), limits()).unwrap();

    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let workspace = set.workspace(&query, 7).unwrap();
    let required = limits().max_working_bytes - set.available_working_bytes().unwrap();
    drop(workspace);
    let exact = IndexSet::new(
        parts(),
        ResourceLimits {
            max_working_bytes: required,
            ..limits()
        },
    )
    .unwrap();
    let ws = exact.workspace(&query, 7).unwrap();
    assert!(exact.workspace(&query, 7).is_err());
    assert!(exact.to_bytes().is_err());
    drop(ws);
    assert!(exact.workspace(&query, 7).is_ok());
    assert!(exact.to_bytes().is_ok());
    let short = IndexSet::new(
        parts(),
        ResourceLimits {
            max_working_bytes: required - 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(short.workspace(&query, 7).is_err());
}

#[test]
fn concurrent_column_planners_cannot_spend_the_same_remaining_capacity() {
    use std::sync::{Arc, Barrier};
    let text = "abcdef".repeat(50);
    let texts = [text.as_str()];
    let rows: Vec<_> = (0..200).map(|key| (key, key, texts.as_slice())).collect();
    let parts = vec![segment(&rows[..100], &[]), segment(&rows[100..], &[])];
    let set = IndexSet::new(
        parts,
        ResourceLimits {
            max_working_bytes: 2 * 1024 * 1024,
            ..limits()
        },
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let barrier = barrier.clone();
            let set = &set;
            scope.spawn(move || {
                let mut query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
                let mut ws = set.workspace(&query, 3).unwrap();
                barrier.wait();
                ws.enable_planner(set, &query, 2 * 1024 * 1024, 4096)
                    .unwrap();
                barrier.wait();
                barrier.wait();
                query.update(&units("abc")).unwrap();
                assert!(
                    !set.search(&query, &mut ws, 3, true)
                        .unwrap()
                        .hits
                        .is_empty()
                );
            });
        }
        barrier.wait();
        barrier.wait();
        // reserve(0) checks the unsaturated sum of all live reservations.
        assert!(set.reserve_working(0).is_ok());
        barrier.wait();
    });
    assert!(set.reserve_working(0).is_ok());
}

#[test]
fn positional_transition_budget_is_shared_across_the_whole_column() {
    let text = "a".repeat(16384);
    let set = IndexSet::new(
        vec![
            segment(&[(0, 0, &[text.as_str()])], &[]),
            segment(&[(1, 1, &[text.as_str()])], &[]),
        ],
        ResourceLimits {
            max_pattern_units: 4096,
            ..ResourceLimits::default()
        },
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
    query.update(&units(&"a".repeat(2049))).unwrap();
    let mut ws = set.workspace(&query, 0).unwrap();

    assert!(matches!(
        set.search(&query, &mut ws, 1, true),
        Err(asearch_engine::SearchError::ResourceLimit(
            "positional ranking transition budget"
        ))
    ));
    assert_eq!(
        set.search(&query, &mut ws, 0, true)
            .unwrap()
            .totals
            .unwrap(),
        [2]
    );

    query.update(&units("a")).unwrap();
    assert_eq!(set.search(&query, &mut ws, 2, true).unwrap().hits.len(), 2);
}
