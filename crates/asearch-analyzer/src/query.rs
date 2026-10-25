//! Builds bounded query rewrite data from native dictionary forms and kana readings.

use crate::{Analyzer, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use sudachi::dic::subset::InfoSubset;
mod kana;
mod target;
use kana::kana_rules;
use target::Target;

/// Versioned query-only tables; no tokenizer or dictionary is needed by consumers.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRules {
    pub format: &'static str,
    pub forms: Vec<[String; 2]>,
    pub kana: Vec<[String; 2]>,
}

/// Selects supported endings before dictionary strings need to be decoded.
fn endings(pos: &[String]) -> &'static [&'static str] {
    let kind = pos.first().map(String::as_str).unwrap_or("");
    let class = pos.get(4).map(String::as_str).unwrap_or("");
    let form = pos.get(5).map(String::as_str).unwrap_or("");
    if form.starts_with("終止形") || form.starts_with("連体形") {
        return &[""];
    }
    if !form.starts_with("連用形") {
        return &[];
    }
    let suffix = if form.contains("撥音便") || (form.contains("イ音便") && class.contains("ガ行"))
    {
        Some("だ")
    } else if form.contains("促音便")
        || form.contains("イ音便")
        || (kind == "動詞"
            && (class.contains("一段") || class.contains("変格") || class.contains("サ行")))
    {
        Some("た")
    } else {
        None
    };
    let polite = kind == "動詞" && form == "連用形-一般";
    match (suffix, polite) {
        (Some("だ"), true) => &["だ", "ました"],
        (Some("だ"), false) => &["だ"],
        (Some(_), true) => &["た", "ました"],
        (Some(_), false) => &["た"],
        (None, true) => &["ました"],
        (None, false) => &[],
    }
}

impl Analyzer {
    /// Exports unambiguous whole-word inflections for vocabulary present in the corpus.
    pub fn query_rules<'a>(
        &mut self,
        texts: impl IntoIterator<Item = &'a str>,
    ) -> Result<QueryRules> {
        let mut vocabulary = BTreeSet::new();
        for text in texts {
            self.tokenize(text)?;
            for token in self.morphemes.iter() {
                let surface = token.surface();
                let forms = [
                    surface.as_ref(),
                    token.normalized_form(),
                    token.dictionary_form(),
                ];
                if matches!(token.part_of_speech()[0].as_str(), "動詞" | "形容詞")
                    && self.eligible(&forms, token.part_of_speech())
                {
                    vocabulary.insert(token.normalized_form().to_owned());
                }
            }
        }
        let dictionary = self.tokenizer.dict().clone();
        let lexicon = dictionary.lexicon();
        let subset = InfoSubset::POS_ID | InfoSubset::HEADWORD | InfoSubset::NORMALIZED_FORM;
        let mut candidates: BTreeMap<String, Target> = BTreeMap::new();
        for id in lexicon.word_ids() {
            let info = lexicon.get_word_info_subset(id?, subset)?;
            let pos = &dictionary.grammar().pos_list[info.pos_id() as usize];
            if !matches!(pos[0].as_str(), "動詞" | "形容詞") {
                continue;
            }
            let endings = endings(pos);
            if endings.is_empty() {
                continue;
            }
            let normalized = info.normalized_form(lexicon);
            let surface = info.headword(lexicon);
            for suffix in endings {
                let value = format!("{surface}{suffix}");
                if value.encode_utf16().count() > 128 {
                    continue;
                }
                candidates
                    .entry(value)
                    .and_modify(|target| target.observe(normalized))
                    .or_insert_with(|| Target::new(normalized));
                if candidates.len() > 250_000 {
                    return Err("query rewrite candidate limit".into());
                }
            }
        }
        let mut forms = Vec::new();
        for (source, target) in candidates {
            let Some(target) = target.unique() else {
                continue;
            };
            if source.is_empty()
                || target.is_empty()
                || source.chars().any(char::is_whitespace)
                || target.chars().any(char::is_whitespace)
                || source == target
                || target.encode_utf16().count() > 128
                || !vocabulary.contains(&target)
            {
                continue;
            }
            self.tokenize(&source)?;
            let mut words = self.morphemes.iter().filter(|token| {
                matches!(
                    token.part_of_speech()[0].as_str(),
                    "動詞" | "形容詞" | "名詞" | "形状詞"
                )
            });
            if words
                .next()
                .is_some_and(|token| token.normalized_form() == target.as_str())
                && words.next().is_none()
            {
                forms.push([source, target]);
                if forms.len() > 50_000 {
                    return Err("query rewrite output limit".into());
                }
            }
        }
        // Canonical targets never rewrite again, regardless of the caller's query history.
        let sources: BTreeSet<_> = forms.iter().map(|pair| pair[0].clone()).collect();
        forms.retain(|pair| !sources.contains(&pair[1]));

        Ok(QueryRules {
            format: "asearch-query-rules/1",
            forms,
            kana: kana_rules(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn past_and_polite_forms_follow_dictionary_conjugation_classes() {
        let pos = |kind: &str, class: &str, form: &str| {
            [kind, "*", "*", "*", class, form].map(str::to_owned)
        };
        assert_eq!(
            endings(&pos("動詞", "下一段-バ行", "連用形-一般")),
            ["た", "ました"]
        );
        for form in ["未然形-一般", "仮定形-一般", "命令形"] {
            assert!(endings(&pos("動詞", "下一段-バ行", form)).is_empty());
        }
        for form in ["終止形-一般", "連体形-一般"] {
            assert_eq!(endings(&pos("動詞", "五段-カ行", form)), [""]);
        }
        assert_eq!(
            endings(&pos("動詞", "五段-カ行", "連用形-一般")),
            ["ました"]
        );
        assert_eq!(endings(&pos("動詞", "五段-マ行", "連用形-撥音便")), ["だ"]);
        assert_eq!(endings(&pos("形容詞", "形容詞", "連用形-促音便")), ["た"]);
        assert_eq!(
            endings(&pos("形容詞", "形容詞", "連用形-一般")),
            Vec::<&str>::new()
        );
    }
}
