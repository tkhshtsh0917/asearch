use asearch_engine::{Asearch, MatchMode, Profile};
use serde_json::Value;

fn units(v: &Value) -> Vec<u16> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as u16)
        .collect()
}

#[test]
fn published_and_source_fixtures() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/compat.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let pattern_units = units(&case["pattern"]);
        let pattern = String::from_utf16(&pattern_units);
        let mode = match case["mode"]["json"].as_u64() {
            Some(1) => MatchMode::Include,
            Some(2) => MatchMode::WildcardSpace,
            _ => MatchMode::Exact,
        };
        if case["ctor"].get("throws").is_some() {
            if let Ok(p) = &pattern {
                assert!(Asearch::new(p, mode).is_err());
            }
            continue;
        }
        let ctor = &case["ctor"]["ok"];
        let masks: Vec<_> = ctor["masks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| {
                let u = units(&entry[0]);
                let mut cps = char::decode_utf16(u.iter().copied())
                    .map(|c| c.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from));
                let cp = cps.next()?;
                if cps.next().is_some() {
                    return None;
                }
                Some((cp, entry[1].as_i64().unwrap() as u32))
            })
            .collect();
        for (profile, key) in [(Profile::AsearchTs, "src"), (Profile::AsearchTsLib, "lib")] {
            let a = Asearch::from_parts(
                &masks,
                ctor["loopMask"].as_i64().unwrap() as u32,
                ctor["acceptPattern"].as_i64().unwrap() as u32,
                ctor["initPattern"].as_i64().unwrap() as u32,
                profile,
            );
            for (ti, text) in case["texts"].as_array().unwrap().iter().enumerate() {
                let text = units(text);
                for (ei, e) in fixture["ambigs"].as_array().unwrap().iter().enumerate() {
                    let e = e.as_u64().unwrap() as u16;
                    let expected = case[key][ti][ei].as_bool().unwrap();
                    assert_eq!(
                        a.is_match_utf16(&text, e),
                        expected,
                        "{} {key} e={e}",
                        case["id"]
                    );
                    if let Ok(p) = &pattern {
                        let native = Asearch::with_profile(p, mode, profile).unwrap();
                        assert_eq!(
                            native.is_match_utf16(&text, e),
                            expected,
                            "native {} {key}",
                            case["id"]
                        );
                        if let Ok(t) = String::from_utf16(&text) {
                            assert_eq!(native.is_match(&t, e), expected);
                        }
                    }
                }
            }
        }
    }
}
