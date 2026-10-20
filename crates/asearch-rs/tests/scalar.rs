use asearch_rs::{BitWord, MatchMode, MultiWord, ScalarError, ScalarLimits, ScalarWm};

mod support;
use support::oracle;

fn compare<W: BitWord>(pattern: &str, text: &str, mode: MatchMode) {
    let compiled = ScalarWm::<W>::new(pattern, mode, ScalarLimits::default()).unwrap();
    let mut workspace = compiled.workspace(7).unwrap();
    let expected = oracle(pattern, text, mode);
    assert_eq!(
        compiled.match_level(text, &mut workspace).unwrap(),
        (expected <= 7).then_some(expected as u16),
        "{pattern:?} {text:?} {mode:?}"
    );
    // Reusing a workspace must reset all states, including empty text.
    let expected = oracle(pattern, "", mode);
    assert_eq!(
        compiled.match_level("", &mut workspace).unwrap(),
        (expected <= 7).then_some(expected as u16)
    );
}

#[test]
fn ideal_matches_independent_cost_matrix() {
    let alphabet = ['a', 'A', 'b', ' ', '😀', '漢', 'ß', 'İ'];
    let mut seed = 31u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as usize
    };
    for _ in 0..2_000 {
        let pattern: String = (0..next() % 18)
            .map(|_| alphabet[next() % alphabet.len()])
            .collect();
        let text: String = (0..next() % 22)
            .map(|_| alphabet[next() % alphabet.len()])
            .collect();
        for mode in [
            MatchMode::Exact,
            MatchMode::Include,
            MatchMode::WildcardSpace,
        ] {
            compare::<u32>(&pattern, &text, mode);
            compare::<u64>(&pattern, &text, mode);
            compare::<u128>(&pattern, &text, mode);
            compare::<MultiWord<3>>(&pattern, &text, mode);
        }
    }
}

#[test]
fn word_boundaries_and_long_patterns() {
    for length in [0, 1, 30, 31, 32, 62, 63, 64, 126, 127, 128, 190, 191] {
        let pattern = "ab".repeat(length / 2) + if length % 2 == 1 { "a" } else { "" };
        for text in [
            pattern.clone(),
            format!("x{pattern}y"),
            format!("{pattern}z"),
        ] {
            for mode in [
                MatchMode::Exact,
                MatchMode::Include,
                MatchMode::WildcardSpace,
            ] {
                compare::<MultiWord<3>>(&pattern, &text, mode);
                if length < 128 {
                    compare::<u128>(&pattern, &text, mode);
                }
                if length < 64 {
                    compare::<u64>(&pattern, &text, mode);
                }
                if length < 32 {
                    compare::<u32>(&pattern, &text, mode);
                }
            }
        }
    }
    let pattern = "a".repeat(65_535);
    let compiled =
        ScalarWm::<MultiWord<1024>>::new(&pattern, MatchMode::Exact, ScalarLimits::default())
            .unwrap();
    let mut workspace = compiled.workspace(1).unwrap();
    assert_eq!(
        compiled.match_level(&pattern, &mut workspace).unwrap(),
        Some(0)
    );
    assert_eq!(
        compiled.match_level(&pattern[1..], &mut workspace).unwrap(),
        Some(1)
    );
}

#[test]
fn carries_and_truncation_do_not_wrap() {
    for bit in 0..191 {
        assert_eq!(
            MultiWord::<3>::bit(bit).shift(),
            MultiWord::<3>::bit(bit + 1)
        );
    }
    assert_eq!(MultiWord::<3>::bit(191).shift(), MultiWord::zero());
    assert_eq!(MultiWord::<0>::bit(0), MultiWord::zero());
    assert_eq!(u32::bit(31).shift(), 0);
    assert_eq!(u64::bit(64), 0);
    assert_eq!(u128::bit(127).shift(), 0);
}

#[test]
fn rejects_limits_without_affecting_existing_workspace() {
    let default = ScalarLimits::default();
    assert!(matches!(
        ScalarWm::<u32>::new(&"a".repeat(32), MatchMode::Exact, default),
        Err(ScalarError::WordCapacity)
    ));
    assert!(matches!(
        ScalarWm::<MultiWord<0>>::new("", MatchMode::Exact, default),
        Err(ScalarError::WordCapacity)
    ));
    assert!(matches!(
        ScalarWm::<u32>::new(
            "😀",
            MatchMode::Exact,
            ScalarLimits {
                max_pattern_units: 1,
                ..default
            }
        ),
        Err(ScalarError::PatternLimit)
    ));
    assert!(matches!(
        ScalarWm::<u32>::new(
            "a",
            MatchMode::Exact,
            ScalarLimits {
                max_working_bytes: 1,
                ..default
            }
        ),
        Err(ScalarError::WorkingLimit)
    ));
    assert!(matches!(
        ScalarWm::<u32>::new(
            "",
            MatchMode::Exact,
            ScalarLimits {
                max_pattern_units: 0,
                ..default
            }
        ),
        Err(ScalarError::InvalidLimits)
    ));

    let compiled = ScalarWm::<u64>::new(
        "a",
        MatchMode::Exact,
        ScalarLimits {
            max_working_bytes: 1024,
            ..default
        },
    )
    .unwrap();
    assert!(matches!(
        compiled.workspace(u16::MAX),
        Err(ScalarError::WorkingLimit)
    ));
    let mut workspace = compiled.workspace(1).unwrap();
    assert_eq!(workspace.max_ambig(), 1);
    assert_eq!(compiled.match_level("a", &mut workspace).unwrap(), Some(0));

    let roomy = ScalarWm::<u64>::new("a", MatchMode::Exact, default).unwrap();
    let mut large = roomy.workspace(u16::MAX).unwrap();
    assert_eq!(
        compiled.match_level("a", &mut large),
        Err(ScalarError::WorkingLimit)
    );
    assert_eq!(roomy.match_level("", &mut large).unwrap(), Some(1));
}
