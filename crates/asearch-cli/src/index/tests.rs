use super::*;
use std::io::Cursor;

#[test]
fn build_is_deterministic_and_searches_normalized_questions() {
    let a = br#"{"key":2,"texts":["ABC"]}
{"key":1,"order":5,"texts":["other","ABC"]}
"#;
    let b = br#"{"key":1,"order":5,"texts":["other","ABC"]}
{"key":2,"texts":["ABC"]}
"#;
    let bytes = build(Cursor::new(a)).unwrap();
    assert_eq!(bytes, build(Cursor::new(b)).unwrap());
    let found = search(&bytes, "ＡＢＣ").unwrap();
    assert_eq!(found["hits"][0]["key"], 2);
    assert_eq!(found["hits"][1]["key"], 1);
    assert_eq!(found["totals"], json!([2]));
}

#[test]
fn jsonl_rejects_malformed_duplicate_and_oversized_documents() {
    for input in [
        r#"{"key":1,"texts":[]}"#,
        r#"{"key":-1,"texts":["a"]}"#,
        r#"{"key":1,"texts":["a"],"unexpected":true}"#,
        "{invalid}",
    ] {
        assert!(build(Cursor::new(input)).is_err());
    }
    assert!(build(Cursor::new(" ".repeat(input::MAX_LINE_BYTES as usize + 1))).is_err());
    assert!(build(Cursor::new("{\"key\":1,\"texts\":[\"a\"]}\n".repeat(2))).is_err());
    assert!(
        build(Cursor::new(format!(
            "{{\"key\":1,\"texts\":[\"{}\"]}}",
            "a".repeat(65536)
        )))
        .is_err()
    );
    assert!(search(b"broken", "a").is_err());
}

#[test]
fn explicit_null_order_is_not_a_missing_default() {
    assert!(
        super::build(std::io::Cursor::new(
            b"{\"key\":1,\"order\":null,\"texts\":[\"a\"]}\n"
        ))
        .is_err()
    );
    assert!(
        super::delta(std::io::Cursor::new(
            b"{\"op\":\"upsert\",\"key\":1,\"order\":null,\"texts\":[\"a\"]}\n"
        ))
        .is_err()
    );
    assert!(super::build(std::io::Cursor::new(b"{\"key\":1,\"texts\":[\"a\"]}\n")).is_ok());
}

#[test]
fn release_notices_cover_every_locked_external_dependency() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lock = include_str!("../../../../Cargo.lock");
    for package in lock.split("[[package]]").skip(1) {
        let field = |name: &str| {
            package
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{name} = \"")))
                .and_then(|value| value.strip_suffix('"'))
        };
        let name = field("name").unwrap();
        if !package.contains("source = \"registry+") {
            continue;
        }
        let version = field("version").unwrap();
        let directory = workspace
            .join("crates/asearch-engine/licenses/third-party")
            .join(format!("{name}-{version}"));
        assert!(
            directory.is_dir(),
            "missing notice directory: {name}-{version}"
        );
        assert!(
            std::fs::read_dir(directory).unwrap().any(|entry| {
                let name = entry.unwrap().file_name();
                let name = name.to_string_lossy();
                name.starts_with("LICENSE") || name == "UNLICENSE" || name == "COPYING"
            }),
            "missing license: {name}-{version}"
        );
    }
}

#[test]
fn search_uses_wire_descriptors_for_index_and_query() {
    use asearch_engine::{Doc, Normalizer, Profile};
    let text: Vec<u16> = "ＡＢＣ".encode_utf16().collect();
    let refs = [text.as_slice()];
    let docs = [Doc {
        key: 1,
        order: 0,
        texts: &refs,
    }];
    for profile in [Profile::Ideal, Profile::AsearchTsLib] {
        for normalizer in [Normalizer::NONE, Normalizer::JA_BASIC] {
            let options = TransposedOptions {
                profile,
                normalizer,
                ..Default::default()
            };
            let bytes =
                asearch_builder::build(&docs, &[], options, ResourceLimits::default()).unwrap();
            assert_eq!(search(&bytes, "ＡＢＣ").unwrap()["hits"][0]["key"], 1);
        }
    }
    assert!(search(&[0; 63], "a").is_err());
}
