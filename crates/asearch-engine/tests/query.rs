use asearch_engine::{AmbigPolicy, MatchMode, QueryOptions, QueryWorkspace, ResourceLimits};

#[test]
fn query_preparation_orders_normalization_policy_and_wrap() {
    let mut query =
        QueryWorkspace::new(QueryOptions::default(), ResourceLimits::default()).unwrap();
    assert_eq!(query.pattern(), [32, 32]);
    for (input, expected, level) in [
        ("ｶﾞｷｸｹ", "がきくけ", 1),
        ("ＡＢＣ", "abc", 0),
        ("ab cd", "ab cd", 1),
    ] {
        query
            .update(&input.encode_utf16().collect::<Vec<_>>())
            .unwrap();
        assert_eq!(
            query.normalized(),
            expected.encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(query.max_ambig(), level);
        assert_eq!(query.pattern().len(), query.normalized().len() + 2);
    }

    for mode in [MatchMode::Exact, MatchMode::Include] {
        let mut query = QueryWorkspace::new(
            QueryOptions {
                mode,
                ..QueryOptions::default()
            },
            ResourceLimits::default(),
        )
        .unwrap();
        query.update(&[0xd800, 0xdc01, 0xdc00]).unwrap();
        assert_eq!(query.pattern(), [0xd800, 0xdc01, 0xdc00]);
    }
}

#[test]
fn failed_queries_preserve_successful_state_and_limits_reject_before_reservation() {
    let limits = ResourceLimits {
        max_pattern_units: 4,
        ..ResourceLimits::default()
    };

    let mut query = QueryWorkspace::new(
        QueryOptions {
            policy: AmbigPolicy::fixed(3).unwrap(),
            ..QueryOptions::default()
        },
        limits,
    )
    .unwrap();
    query.update(&[0x61; 4]).unwrap();
    let old = query.pattern().to_vec();
    assert_eq!(
        query.update(&[0x61; 5]).unwrap_err().code(),
        "RESOURCE_LIMIT"
    );
    assert_eq!(query.pattern(), old);
    assert_eq!(query.max_ambig(), 3);
    let exact_budget = query.capacity_bytes();
    assert!(
        QueryWorkspace::new(
            QueryOptions::default(),
            ResourceLimits {
                max_working_bytes: exact_budget,
                ..limits
            }
        )
        .is_ok()
    );
    assert!(
        QueryWorkspace::new(
            QueryOptions::default(),
            ResourceLimits {
                max_working_bytes: exact_budget - 1,
                ..limits
            }
        )
        .is_err()
    );
    for bad in [
        ResourceLimits {
            max_texts: 0,
            ..limits
        },
        ResourceLimits {
            max_segments: 9,
            ..limits
        },
        ResourceLimits {
            max_pattern_units: usize::MAX,
            ..limits
        },
    ] {
        assert!(QueryWorkspace::new(QueryOptions::default(), bad).is_err());
    }
}
