use asearch_rs::{AsearchIndex, Doc, Normalizer, ResourceLimits, TransposedOptions};
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let directory = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected corpus directory")?,
    );
    fs::create_dir_all(&directory)?;
    let a: Vec<_> = "ｶﾞＡ😀 a b".encode_utf16().collect();
    let b = [0xd800, 0x61];
    let texts = [a.as_slice(), b.as_slice(), &[]];
    let docs = [Doc {
        key: 9,
        order: 2,
        texts: &texts,
    }];
    for (name, normalizer) in [("ja", Normalizer::JA_BASIC), ("none", Normalizer::NONE)] {
        let options = TransposedOptions {
            normalizer,
            block_words: 1,
            ..TransposedOptions::default()
        };
        fs::write(
            directory.join(format!("{name}.asix")),
            AsearchIndex::from_docs(&docs, options, ResourceLimits::default())?.to_bytes()?,
        )?;
        fs::write(
            directory.join(format!("{name}-empty.asix")),
            AsearchIndex::from_docs(&[], options, ResourceLimits::default())?.to_bytes()?,
        )?;
    }
    Ok(())
}
