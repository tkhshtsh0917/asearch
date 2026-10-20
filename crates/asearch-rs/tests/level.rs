use asearch_rs::{Asearch, InvalidMaxAmbig, MatchMode, Profile};

#[test]
fn minimum_level_handles_profiles_empty_inputs_and_bounds() {
    let a = Asearch::new("a", MatchMode::Exact).unwrap();
    assert_eq!(a.match_level("", 0), Ok(None));
    assert_eq!(a.match_level("", 3), Ok(Some(1)));
    assert_eq!(a.match_level("a", 31), Ok(Some(0)));
    assert_eq!(a.match_level("", 32), Err(InvalidMaxAmbig(32)));
    assert_eq!(
        a.match_level_utf16(&[], u16::MAX),
        Err(InvalidMaxAmbig(u16::MAX))
    );
    for profile in [Profile::AsearchTs, Profile::AsearchTsLib] {
        for mode in [
            MatchMode::Exact,
            MatchMode::Include,
            MatchMode::WildcardSpace,
        ] {
            for pattern in ["", " ", "a b", "😀", "漢字", "ß"] {
                let a = Asearch::with_profile(pattern, mode, profile).unwrap();
                for text in ["", "ab", "a漢字b", "😀", "漢字"] {
                    let units: Vec<_> = text.encode_utf16().collect();
                    for max in 0..=31 {
                        let expected = (0..=max).find(|&e| a.is_match(text, e));
                        assert_eq!(a.match_level(text, max), Ok(expected));
                        assert_eq!(a.match_level_utf16(&units, max), Ok(expected));
                    }
                }
            }
        }
    }
    let surrogate = Asearch::from_parts(
        &[(0xd83d, 1 << 31)],
        0,
        1 << 30,
        1 << 31,
        Profile::AsearchTs,
    );
    assert_eq!(surrogate.match_level_utf16(&[0xd83d], 0), Ok(Some(0)));
}
