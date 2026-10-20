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
