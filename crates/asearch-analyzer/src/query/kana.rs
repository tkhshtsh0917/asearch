//! Materializes kana conversion as lookup data for bounded client rewrites.

use std::collections::{BTreeMap, BTreeSet};
use wana_kana::ConvertJapanese;

/// Materializes the romanizer as data so the browser only performs bounded lookups.
pub(super) fn kana_rules() -> Vec<[String; 2]> {
    let singles: Vec<String> = (0x3041..=0x3096)
        .chain(0x30A1..=0x30FA)
        .filter_map(char::from_u32)
        .map(|c| c.to_string())
        .collect();
    let mut inputs: BTreeSet<String> = singles.iter().cloned().collect();
    for first in &singles {
        for second in "ぁぃぅぇぉゃゅょゎァィゥェォャュョヮ".chars() {
            inputs.insert(format!("{first}{second}"));
        }
    }
    let syllables: Vec<_> = inputs
        .iter()
        .cloned()
        .chain(('a'..='z').map(|c| c.to_string()))
        .collect();
    for prefix in ["っ", "ッ", "ん", "ン"] {
        for value in &syllables {
            inputs.insert(format!("{prefix}{value}"));
        }
    }
    let mut inputs: Vec<_> = inputs.into_iter().collect();
    inputs.sort_by_key(|value| value.chars().count());
    let mut output = BTreeMap::new();
    for value in inputs {
        let romanized = value.to_romaji();
        if !romanized.is_empty() && romanized.is_ascii() && romanized != apply_kana(&value, &output)
        {
            output.insert(value, romanized);
        }
    }
    output.insert("ー".into(), "-".into());
    output.into_iter().map(|(a, b)| [a, b]).collect()
}

fn apply_kana(value: &str, rules: &BTreeMap<String, String>) -> String {
    let chars: Vec<_> = value.chars().collect();
    let mut result = String::new();
    let mut index = 0;
    while index < chars.len() {
        let found = (1..=3.min(chars.len() - index)).rev().find_map(|length| {
            rules
                .get(&chars[index..index + length].iter().collect::<String>())
                .map(|text| (length, text))
        });
        if let Some((length, text)) = found {
            result.push_str(text);
            index += length;
        } else {
            result.push(chars[index]);
            index += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_kana_table_contains_mixed_ime_prefixes_and_long_marks() {
        let rules: BTreeMap<_, _> = kana_rules().into_iter().map(|[a, b]| (a, b)).collect();
        assert_eq!(rules.get("び").unwrap(), "bi");
        assert_eq!(rules.get("ビ").unwrap(), "bi");
        assert_eq!(rules.get("ー").unwrap(), "-");
        assert_eq!(rules.get("きゃ").unwrap(), "kya");
        assert_eq!(apply_kana("っきゃ", &rules), "kkya");
        assert_eq!(apply_kana("びーt", &rules), "bi-t");
        assert!(rules.len() <= 8192);
    }
}
