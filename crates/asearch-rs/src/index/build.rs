use super::{AsearchIndex, Doc, Document, Text};
use crate::{
    ResourceLimits, SearchError, TransposedEngine, TransposedOptions,
    capacity::{Budget, add, mul},
};
use std::mem::size_of;

pub(super) fn build(
    docs: &[Doc<'_>],
    options: TransposedOptions,
    limits: ResourceLimits,
) -> Result<AsearchIndex, SearchError> {
    limits.validate()?;
    let mut count = 0;
    let mut units = 0;
    let mut longest = 0;
    for doc in docs {
        if doc.texts.is_empty() {
            return Err(SearchError::InvalidInput(
                "a document requires at least one text",
            ));
        }
        count = add(count, doc.texts.len())?;
        if count > limits.max_texts {
            return Err(SearchError::ResourceLimit("index text limit"));
        }
        for text in doc.texts {
            if text.len() > u16::MAX as usize {
                return Err(SearchError::ResourceLimit(
                    "text exceeds 65535 UTF-16 units",
                ));
            }
            units = add(units, text.len())?;
            longest = longest.max(text.len());
        }
    }
    // Both retained buffers reserve the raw unit count. Normalization cannot
    // expand, and unused capacity still belongs in the instance's budget.
    if mul(units, 4)? > limits.max_total_bytes {
        return Err(SearchError::ResourceLimit(
            "index retained input byte limit",
        ));
    }
    let mut budget = Budget::new(
        size_of::<AsearchIndex>() - size_of::<TransposedEngine>(),
        limits.max_working_bytes,
    )?;
    let mut sorted: Vec<&Doc<'_>> = budget.reserve(docs.len())?;
    sorted.extend(docs.iter());
    sorted.sort_unstable_by_key(|doc| doc.key);
    if sorted.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(SearchError::InvalidInput("duplicate document key"));
    }
    sorted.sort_unstable_by_key(|doc| (doc.order, doc.key));
    let mut documents = budget.reserve(docs.len())?;
    let mut texts = budget.reserve(count)?;
    let mut original = budget.reserve(units)?;
    let mut normalized = budget.reserve(units)?;
    if mul(add(original.capacity(), normalized.capacity())?, 2)? > limits.max_total_bytes {
        return Err(SearchError::ResourceLimit(
            "index retained input byte limit",
        ));
    }
    let mut scratch = budget.reserve(longest)?;
    for doc in &sorted {
        let start = texts.len();
        for text in doc.texts {
            options
                .normalizer
                .normalize_into(text, &mut scratch)
                .map_err(|_| SearchError::ResourceLimit("normalization capacity"))?;
            if add(normalized.len(), scratch.len())? > limits.max_units {
                return Err(SearchError::ResourceLimit("index normalized unit limit"));
            }
            texts.push(Text {
                original_start: original.len(),
                original_len: text.len(),
                normalized_start: normalized.len(),
                normalized_len: scratch.len(),
            });
            original.extend_from_slice(text);
            normalized.extend_from_slice(&scratch);
        }
        documents.push(Document {
            key: doc.key,
            order: doc.order,
            start,
            end: texts.len(),
        });
    }
    budget.release(scratch);
    budget.release(sorted);
    let mut refs = budget.reserve(count)?;
    refs.extend(
        texts
            .iter()
            .map(|text| &original[text.original_start..text.original_start + text.original_len]),
    );
    let mut engine = TransposedEngine::new(
        &refs,
        options,
        ResourceLimits {
            max_working_bytes: limits.max_working_bytes - budget.used,
            ..limits
        },
    )?;
    budget.release(refs);
    engine.set_working_budget(limits.max_working_bytes - budget.used);
    let capacity_bytes = add(budget.used, engine.capacity_bytes())?;
    Ok(AsearchIndex {
        engine,
        documents,
        texts,
        original,
        normalized,
        limits,
        capacity_bytes,
    })
}
