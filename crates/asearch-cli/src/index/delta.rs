use super::input::{Documents, InputDoc, records};
use crate::Result;
use asearch_rs::ResourceLimits;
use serde::Deserialize;
use std::{collections::BTreeSet, io::BufRead};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Change {
    Upsert {
        key: u32,
        #[serde(default, deserialize_with = "super::input::optional_order")]
        order: Option<u32>,
        texts: Vec<String>,
    },
    Remove {
        key: u32,
    },
}

pub fn build(reader: impl BufRead) -> Result<Vec<u8>> {
    let mut documents = Documents::default();
    let mut keys = BTreeSet::new();
    records(reader, |change: Change| {
        let key = match &change {
            Change::Upsert { key, .. } | Change::Remove { key } => *key,
        };
        if keys.len() >= ResourceLimits::default().max_texts {
            return Err("too many change records".into());
        }
        if !keys.insert(key) {
            return Err("duplicate change key".into());
        }
        if let Change::Upsert { key, order, texts } = change {
            documents.push(InputDoc { key, order, texts })?;
        }
        Ok(())
    })?;
    let mut tombstones = Vec::new();
    tombstones.try_reserve_exact(keys.len())?;
    tombstones.extend(keys);
    documents.build(&tombstones)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asearch_rs::{AsearchIndex, TransposedOptions};
    use std::io::Cursor;
    #[test]
    fn upserts_and_removals_always_invalidate_the_old_document() {
        let changes = br#"{"op":"upsert","key":2,"order":3,"texts":["ABC"]}
{"op":"remove","key":9}
"#;
        let bytes = build(Cursor::new(changes)).unwrap();
        let index = AsearchIndex::from_bytes(
            &bytes,
            TransposedOptions::default(),
            ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(index.tombstones(), &[2, 9]);
        assert_eq!(index.document_count(), 1);
        assert_eq!(index.text(0), Some(&[97, 98, 99][..]));
        for bad in [
            r#"{"op":"remove","key":9,"texts":["x"]}"#,
            r#"{"op":"upsert","key":9,"texts":[]}"#,
            r#"{"op":"unknown","key":9}"#,
            "{\"op\":\"remove\",\"key\":1}\n{\"op\":\"remove\",\"key\":1}",
        ] {
            assert!(build(Cursor::new(bad)).is_err());
        }
    }
}
