use asearch_rs::{Asearch, MatchMode, Profile};

#[test]
fn exact_matching_accepts_one_deleted_character() {
    let pattern = Asearch::new("hello", MatchMode::Exact).unwrap();
    assert!(pattern.is_match("helo", 1));
}

#[test]
fn text_iteration_profiles_distinguish_surrogate_pairs() {
    let code_points = Asearch::new("😀", MatchMode::Exact).unwrap();
    let code_units = Asearch::with_profile("😀", MatchMode::Exact, Profile::AsearchTsLib).unwrap();
    assert!(code_points.is_match("😀", 0));
    assert!(!code_units.is_match("😀", 0));
}

#[test]
fn compatibility_levels_are_nonmonotonic() {
    let pattern = Asearch::new("a", MatchMode::Exact).unwrap();
    assert!(!pattern.is_match("", 0));
    assert!(pattern.is_match("", 1));
    assert!(!pattern.is_match("", 2));
}
