use crate::Result;
use asearch_engine::{Asearch, Profile};
use serde_json::{Value, json};

fn units(value: &Value) -> Result<Vec<u16>> {
    value
        .as_array()
        .ok_or("expected UTF-16 array")?
        .iter()
        .map(|unit| {
            unit.as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(|| "invalid UTF-16 unit".into())
        })
        .collect()
}

fn word(value: &Value) -> Result<u32> {
    let value = value.as_i64().ok_or("expected state word")?;
    if !(i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&value) {
        return Err("state word out of range".into());
    }
    Ok(value as u32)
}

fn masks(ctor: &Value) -> Result<Vec<(u32, u32)>> {
    let mut masks = Vec::new();
    for entry in ctor["masks"].as_array().ok_or("missing masks")? {
        let units = units(&entry[0])?;
        let mut symbols = char::decode_utf16(units)
            .map(|c| c.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from));
        if let Some(symbol) = symbols.next().filter(|_| symbols.next().is_none()) {
            masks.push((symbol, word(&entry[1])?));
        }
    }
    Ok(masks)
}

pub fn verify(fixture: &Value) -> Result<Value> {
    let cases = fixture["cases"].as_array().ok_or("missing fixture cases")?;
    let ambigs: Vec<u16> = fixture["ambigs"]
        .as_array()
        .ok_or("missing levels")?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(|| "invalid fixture level".into())
        })
        .collect::<Result<_>>()?;
    let mut assertions = 0usize;
    let mut skipped_constructors = 0;
    for case in cases {
        if case["ctor"].get("throws").is_some() {
            skipped_constructors += 1;
            continue;
        }
        let ctor = &case["ctor"]["ok"];
        let masks = masks(ctor)?;
        let texts = case["texts"].as_array().ok_or("missing fixture texts")?;
        for (profile, key) in [(Profile::AsearchTs, "src"), (Profile::AsearchTsLib, "lib")] {
            let matcher = Asearch::from_parts(
                &masks,
                word(&ctor["loopMask"])?,
                word(&ctor["acceptPattern"])?,
                word(&ctor["initPattern"])?,
                profile,
            );
            let expected = case[key].as_array().ok_or("missing expected results")?;
            if expected.len() != texts.len() {
                return Err("fixture text/result count differs".into());
            }
            for (text, expected) in texts.iter().zip(expected) {
                let text = units(text)?;
                let expected = expected.as_array().ok_or("expected result array")?;
                if expected.len() != ambigs.len() {
                    return Err("fixture level/result count differs".into());
                }
                for (&level, expected) in ambigs.iter().zip(expected) {
                    let expected = expected.as_bool().ok_or("expected boolean result")?;
                    if matcher.is_match_utf16(&text, level) != expected {
                        return Err(format!(
                            "fixture mismatch: {} {key} level={level}",
                            case["id"]
                        )
                        .into());
                    }
                    assertions += 1;
                }
            }
        }
    }
    if assertions == 0 {
        return Err("fixture contains no match assertions".into());
    }
    Ok(
        json!({ "cases": cases.len(), "skippedConstructors": skipped_constructors, "matchAssertions": assertions }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_real_fixtures_and_detects_a_corrupted_expected_bit() {
        let mut fixture: Value = serde_json::from_str(include_str!(
            "../../asearch-engine/tests/fixtures/compat.json"
        ))
        .unwrap();
        let summary = verify(&fixture).unwrap();
        assert_eq!(summary["cases"], 3168);
        assert!(summary["matchAssertions"].as_u64().unwrap() > 100_000);
        let case = fixture["cases"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|case| case["src"][0][0].is_boolean())
            .unwrap();
        case["src"][0][0] = Value::Bool(!case["src"][0][0].as_bool().unwrap());
        assert!(
            verify(&fixture)
                .unwrap_err()
                .to_string()
                .contains("fixture mismatch")
        );
        assert!(verify(&json!({ "cases": [], "ambigs": [] })).is_err());
    }
}
