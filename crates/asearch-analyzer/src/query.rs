//! Builds bounded query rewrite data from native dictionary forms and kana readings.

use crate::{Analyzer, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use sudachi::dic::subset::InfoSubset;
mod kana;
use kana::kana_rules;

/// Versioned query-only tables; no tokenizer or dictionary is needed by consumers.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRules {
    pub format: &'static str,
    pub forms: Vec<[String; 2]>,
    pub kana: Vec<[String; 2]>,
}

fn variants(surface: &str, pos: &[String]) -> Vec<String> {
    let kind = pos.first().map(String::as_str).unwrap_or("");
    let class = pos.get(4).map(String::as_str).unwrap_or("");
    let form = pos.get(5).map(String::as_str).unwrap_or("");
    let mut result = Vec::new();
    if form.starts_with("終止形") || form.starts_with("連体形") {
        result.push(surface.to_owned());
    }
    if !form.starts_with("連用形") {
        return result;
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
    if let Some(suffix) = suffix {
        result.push(format!("{surface}{suffix}"));
    }
    if kind == "動詞" && form == "連用形-一般" {
        result.push(format!("{surface}ました"));
    }
    result
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
        let mut candidates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for id in lexicon.word_ids() {
            let info = lexicon.get_word_info_subset(id?, subset)?;
            let pos = &dictionary.grammar().pos_list[info.pos_id() as usize];
            if !matches!(pos[0].as_str(), "動詞" | "形容詞") {
                continue;
            }
            let normalized = info.normalized_form(lexicon);
            for value in variants(info.headword(lexicon), pos) {
                if value.encode_utf16().count() > 128 {
                    continue;
                }
                candidates
                    .entry(value)
                    .or_default()
                    .insert(normalized.to_owned());
                if candidates.len() > 250_000 {
                    return Err("query rewrite candidate limit".into());
                }
            }
        }
        let mut forms = Vec::new();
        for (source, targets) in candidates {
            if targets.len() != 1 {
                continue;
            }
            let target = targets.into_iter().next().unwrap();
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
            let words: Vec<_> = self
                .morphemes
                .iter()
                .filter(|token| {
                    matches!(
                        token.part_of_speech()[0].as_str(),
                        "動詞" | "形容詞" | "名詞" | "形状詞"
                    )
                })
                .map(|token| token.normalized_form().to_owned())
                .collect();
            if words == [target.clone()] {
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
            variants("食べ", &pos("動詞", "下一段-バ行", "連用形-一般")),
            ["食べた", "食べました"]
        );
        assert_eq!(
            variants("飲ん", &pos("動詞", "五段-マ行", "連用形-撥音便")),
            ["飲んだ"]
        );
        assert_eq!(
            variants("高かっ", &pos("形容詞", "形容詞", "連用形-促音便")),
            ["高かった"]
        );
        assert_eq!(
            variants("高く", &pos("形容詞", "形容詞", "連用形-一般")),
            Vec::<String>::new()
        );
    }
}
