use asearch_rs::{AmbigPolicy, LengthUnit, MatchMode, Normalizer, Profile};

#[test]
fn default_policy_uses_normalized_input_and_excludes_only_wildcard_spaces() {
    let policy = AmbigPolicy::default();
    for (raw, expected) in [
        ("", 0),
        ("abc", 0),
        ("abcd", 1),
        ("abcdefg", 1),
        ("abcdefgh", 2),
        ("abcdefghijk", 2),
        ("abcdefghijkl", 3),
        ("ＡＢＣＤ", 1),
        ("ｶﾞｷｸｹ", 1),
        ("ab cd", 1),
        ("    ", 0),
    ] {
        let input: Vec<_> = raw.encode_utf16().collect();
        let mut normalized = Vec::with_capacity(input.len());
        Normalizer::JA_BASIC
            .normalize_into(&input, &mut normalized)
            .unwrap();
        assert_eq!(
            policy.evaluate(&normalized, MatchMode::WildcardSpace, Profile::Ideal),
            expected,
            "{raw}"
        );
    }
    for mode in [MatchMode::Exact, MatchMode::Include] {
        assert_eq!(policy.evaluate(&[0x20; 4], mode, Profile::Ideal), 1);
    }
}

#[test]
fn profiles_units_and_invalid_descriptors() {
    let emoji: Vec<_> = "😀😀".encode_utf16().collect();
    for profile in [Profile::AsearchTs, Profile::Ideal] {
        assert_eq!(
            AmbigPolicy::default().evaluate(&emoji, MatchMode::Exact, profile),
            0
        );
    }
    assert_eq!(
        AmbigPolicy::default().evaluate(&emoji, MatchMode::Exact, Profile::AsearchTsLib),
        1
    );
    for unit in [LengthUnit::Utf16, LengthUnit::CodePoint] {
        let policy = AmbigPolicy::by_length(1, 7, unit, false).unwrap();
        assert_eq!(
            policy.evaluate(&emoji, MatchMode::Exact, Profile::Ideal),
            if unit == LengthUnit::Utf16 { 4 } else { 2 }
        );
        assert_eq!(
            policy.evaluate(&[0xd800, 0x61, 0xdc00], MatchMode::Exact, Profile::Ideal),
            3
        );
    }
    let spaces = AmbigPolicy::by_length(4, 3, LengthUnit::Profile, true).unwrap();
    assert_eq!(
        spaces.evaluate(&[0x20; 4], MatchMode::WildcardSpace, Profile::Ideal),
        1
    );
    for level in 0..=7 {
        assert_eq!(
            AmbigPolicy::fixed(level)
                .unwrap()
                .evaluate(&[], MatchMode::Exact, Profile::Ideal),
            level
        );
    }
    assert!(AmbigPolicy::fixed(8).is_err());
    assert!(AmbigPolicy::by_length(0, 3, LengthUnit::Profile, false).is_err());
    assert!(AmbigPolicy::by_length(4, 8, LengthUnit::Profile, false).is_err());
    #[cfg(target_pointer_width = "64")]
    assert!(AmbigPolicy::by_length(usize::MAX, 3, LengthUnit::Profile, false).is_err());
}
