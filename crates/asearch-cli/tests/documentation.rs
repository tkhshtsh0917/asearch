//! Keeps English sources and Japanese translations aligned without Node tooling.
use std::{
    fs,
    path::{Path, PathBuf},
};

fn markdown(directory: &Path, files: &mut Vec<PathBuf>) {
    for item in fs::read_dir(directory).unwrap() {
        let path = item.unwrap().path();
        if path.is_dir() {
            markdown(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
}

fn blocks(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(language) = line.strip_prefix("```") {
            if let Some(block) = current.take() {
                result.push(block);
            } else {
                current = Some((language.to_owned(), String::new()));
            }
        } else if let Some((_, body)) = &mut current
            && !line.trim_start().starts_with('#')
            && !line.trim_start().starts_with("//")
        {
            body.push_str(line);
            body.push('\n');
        }
    }
    assert!(current.is_none(), "unterminated code block");
    result
}

fn prose(text: &str) -> Vec<String> {
    let mut fenced = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced || line.starts_with("<a id=") || line.starts_with("[日本語") {
            continue;
        }
        let mut inline = false;
        lines.push(
            line.chars()
                .filter(|&c| {
                    if c == '`' {
                        inline = !inline;
                        false
                    } else {
                        !inline
                    }
                })
                .collect(),
        );
    }
    lines
}

fn check(source: &str, translated: &str) {
    let lines = prose(source);
    for line in &lines {
        let japanese = line
            .chars()
            .filter(|&c| matches!(c,'\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}'))
            .count();
        let latin = line.chars().filter(char::is_ascii_alphabetic).count();
        assert!(
            !line.contains('。') && japanese <= latin,
            "Japanese prose in English source: {line}"
        );
    }
    let headings = |text: &str| {
        prose(text)
            .into_iter()
            .filter_map(|line| {
                let count = line.bytes().take_while(|&c| c == b'#').count();
                (count > 0 && line.as_bytes().get(count) == Some(&b' ')).then_some(count)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(headings(source), headings(translated), "heading structure");
    let a = blocks(source);
    let b = blocks(translated);
    assert_eq!(a.len(), b.len(), "code block count");
    for ((language, example), (other, translation)) in a.iter().zip(b) {
        assert_eq!(*language, other, "code block language");
        if language != "text" {
            assert_eq!(*example, translation, "executable example");
        }
    }
}

#[test]
fn english_sources_and_japanese_translations_share_structure_and_examples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let docs = root.join("docs");
    let japanese = docs.join("ja");
    let mut files = vec![root.join("README.md")];
    markdown(&docs, &mut files);
    for file in files {
        if let Ok(relative) = file.strip_prefix(&japanese) {
            assert!(
                docs.join(relative).is_file(),
                "orphan translation: {}",
                file.display()
            );
            continue;
        }
        let translation = if file == root.join("README.md") {
            root.join("README.ja.md")
        } else {
            japanese.join(file.strip_prefix(&docs).unwrap())
        };
        let a = fs::read_to_string(&file).unwrap();
        let b = fs::read_to_string(&translation).unwrap();
        check(&a, &b);
    }
}

#[test]
#[should_panic(expected = "Japanese prose")]
fn rejects_japanese_source_even_if_a_translation_exists() {
    check("# 検索\n\n日本語の本文です。", "# 検索\n");
}
