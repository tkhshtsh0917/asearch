//! Server-side Japanese search forms; the browser receives only serialized ASIX texts.

#![forbid(unsafe_code)]

mod policy;
mod resources;
mod texts;

use std::{path::Path, rc::Rc};
use sudachi::{
    analysis::{Mode, stateful_tokenizer::StatefulTokenizer},
    config::Config,
    dic::dictionary::JapaneseDictionary,
    prelude::MorphemeList,
};
use wana_kana::ConvertJapanese;

pub use policy::Policy;
pub use resources::Resources;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Maximum source UTF-8 byte length accepted before Sudachi input rewriting.
pub const MAX_SOURCE_BYTES: usize = 49_149;

/// Reuses one native tokenizer and directed lookup across documents.
pub struct Analyzer {
    tokenizer: StatefulTokenizer<Rc<JapaneseDictionary>>,
    morphemes: MorphemeList<Rc<JapaneseDictionary>>,
    resources: Resources,
    lookup: resources::Lookup,
    policy: Policy,
}

impl Analyzer {
    /// Uses embedded built-in plugins with an explicit, immutable system dictionary path.
    pub fn new(dictionary: &Path, resources: Resources, policy: Policy) -> Result<Self> {
        let config = Config::new(None, None, Some(dictionary.to_owned()))?;
        Self::from_config(config, resources, policy)
    }

    fn from_config(config: Config, resources: Resources, policy: Policy) -> Result<Self> {
        policy.validate()?;
        resources.validate()?;
        let dictionary = Rc::new(JapaneseDictionary::from_cfg(&config)?);
        let mode = match policy.split_mode.as_str() {
            "A" => Mode::A,
            "B" => Mode::B,
            _ => Mode::C,
        };
        Ok(Self {
            tokenizer: StatefulTokenizer::new(dictionary.clone(), mode),
            morphemes: MorphemeList::empty(dictionary),
            lookup: resources::Lookup::new(&resources),
            resources,
            policy,
        })
    }

    /// Keeps the original first, followed by filtered readings and independent synonym terms.
    pub fn analyze(&mut self, text: &str) -> Result<Vec<String>> {
        if text.trim().is_empty() || text.encode_utf16().count() > 65535 {
            return Err("source text is empty or exceeds ASIX text capacity".into());
        }
        if text.len() > MAX_SOURCE_BYTES {
            return Err(format!(
                "source exceeds Sudachi input capacity of {MAX_SOURCE_BYTES} UTF-8 bytes"
            )
            .into());
        }

        self.tokenizer.reset().push_str(text);
        self.tokenizer.do_tokenize()?;
        self.morphemes.collect_results(&mut self.tokenizer)?;

        let mut normalized = Vec::new();
        let mut readings = Vec::new();
        let mut triggers = Vec::new();
        for token in self.morphemes.iter() {
            let surface = token.surface();
            let forms = [
                surface.as_ref(),
                token.normalized_form(),
                token.dictionary_form(),
            ];
            let pos = token.part_of_speech();
            if !self.policy.keep_part_of_speech.contains(&pos[0])
                || self
                    .resources
                    .stoptags
                    .iter()
                    .any(|tag| pos.starts_with(tag))
                || forms
                    .iter()
                    .any(|form| self.resources.stopwords.contains(&form.to_lowercase()))
            {
                continue;
            }

            normalized.push(token.normalized_form().to_owned());
            readings.push(token.reading_form().to_owned());
            triggers.push(forms.map(str::to_owned));
        }

        let reading = readings.join(" ");
        let mut texts = texts::Texts::new(&self.policy);
        for value in [text.to_owned(), normalized.join(" "), reading.clone()]
            .into_iter()
            .chain(romanizations(&reading))
        {
            texts.push(&value)?;
        }

        let mut synonyms = 0;
        for forms in triggers {
            let forms = forms.each_ref().map(String::as_str);
            for alternative in self.lookup.alternatives(&forms) {
                if !self
                    .resources
                    .stopwords
                    .contains(&alternative.to_lowercase())
                    && texts.push(alternative)?
                {
                    synonyms += 1;
                    if Policy::exceeds(self.policy.max_synonym_texts, synonyms) {
                        return Err("question exceeds synonym expansion limit".into());
                    }
                }
            }
        }
        Ok(texts.into_vec())
    }
}

/// Produces repeated-vowel and explicit-long-mark forms without browser dependencies.
pub fn romanizations(reading: &str) -> [String; 2] {
    [
        reading.to_romaji(),
        reading
            .split('ー')
            .map(ConvertJapanese::to_romaji)
            .collect::<Vec<_>>()
            .join("-"),
    ]
}

#[cfg(test)]
mod tests;
