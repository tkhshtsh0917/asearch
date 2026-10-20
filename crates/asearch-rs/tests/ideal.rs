use asearch_rs::{Asearch, MatchMode, Profile};

mod support;

#[test]
fn ideal_levels_are_monotonic_and_equal_the_cost_matrix() {
    for mode in [
        MatchMode::Exact,
        MatchMode::Include,
        MatchMode::WildcardSpace,
    ] {
        for pattern in ["", " ", " a  b ", "a", "😀漢", "ßİΣ", "abc"] {
            let ideal = Asearch::with_profile(pattern, mode, Profile::Ideal).unwrap();
            let compat = Asearch::with_profile(pattern, mode, Profile::AsearchTs).unwrap();
            for text in ["", "ab", " aXb ", "abc", "ss", "i", "ς", "😀漢", "x😀y"] {
                let units: Vec<_> = text.encode_utf16().collect();
                let cost = support::oracle(pattern, text, mode);
                for level in 0..=35 {
                    assert_eq!(ideal.is_match(text, level), cost <= usize::from(level));
                    assert_eq!(
                        ideal.is_match_utf16(&units, level),
                        cost <= usize::from(level)
                    );
                }
                for max in 0..=31 {
                    let expected = (cost <= usize::from(max)).then_some(cost as u16);
                    assert_eq!(ideal.match_level(text, max), Ok(expected));
                    assert_eq!(ideal.match_level_utf16(&units, max), Ok(expected));
                    assert_eq!(ideal.match_level(text, max), compat.match_level(text, max));
                }
            }
        }
    }
}

#[test]
fn ideal_keeps_lone_surrogates_and_reaches_high_levels_without_wrapping() {
    let matcher = Asearch::from_parts(&[(0xd800, 1 << 31)], 0, 1 << 30, 1 << 31, Profile::Ideal);
    assert_eq!(matcher.match_level_utf16(&[0xd800], 0), Ok(Some(0)));
    assert!(!matcher.is_match_utf16(&[0xdc00], 0));
    assert!(matcher.is_match_utf16(&[], u16::MAX));
    assert!(matcher.is_match_utf16(&[0xdc00], u16::MAX));
}
