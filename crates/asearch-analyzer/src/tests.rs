use super::*;
use sudachi::config::PathResolver;

fn policy() -> Policy {
    Policy {
        split_mode: "C".into(),
        keep_part_of_speech: vec![
            "名詞".into(),
            "動詞".into(),
            "形容詞".into(),
            "形状詞".into(),
        ],
        max_synonym_texts: 256,
        max_document_units: 131072,
    }
}

fn analyzer(resources: Resources, policy: Policy) -> Analyzer {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/system.dic");
    let config = Config::minimal_at(PathResolver::from_embedded()).with_system_dic(path);
    Analyzer::from_config(config, resources, policy).unwrap()
}

#[test]
fn romanizations_cover_both_long_vowel_forms() {
    assert_eq!(romanizations("ビートルズ"), ["biitoruzu", "bi-toruzu"]);
    assert_eq!(romanizations("チャーハン"), ["chaahan", "cha-han"]);
}

#[test]
fn deleted_and_output_only_entries_respect_each_groups_direction() {
    let csv = "000001,1,0,1,0,0,0,(),入口,,\n000001,1,1,2,0,0,0,(),出口,,\n000001,1,2,3,0,0,0,(),削除済み,,\n000002,1,0,1,0,0,0,(),出口,,\n000002,1,0,2,0,0,0,(),別グループ,,\n";
    let resource = Resources::compile("# comment\n", "", csv).unwrap();
    let lookup = resources::Lookup::new(&resource);
    assert_eq!(
        lookup.alternatives(&["入口"]).collect::<Vec<_>>(),
        ["入口", "出口"]
    );
    assert_eq!(
        lookup.alternatives(&["削除済み"]).collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
    assert_eq!(
        lookup.alternatives(&["出口"]).collect::<Vec<_>>(),
        ["出口", "別グループ"]
    );
}

#[test]
fn csv_quoting_omitted_flags_and_case_insensitive_triggers_are_supported() {
    let source = "000001,,,,,,,,\"A,B\",,\n000001,,,,,,,,alpha,,\n";
    let resources = Resources::compile("こと\n# comment\n", "助詞,格助詞\n", source).unwrap();
    assert_eq!(
        resources::Lookup::new(&resources)
            .alternatives(&["ALPHA"])
            .collect::<Vec<_>>(),
        ["A,B", "alpha"]
    );
    for source in ["broken", "000001,1,9,1,0,0,0,(),word,,"] {
        assert!(Resources::compile("", "", source).is_err());
    }
}

#[test]
fn native_tokenization_keeps_the_original_and_adds_readings_and_synonyms() {
    let resources = Resources::compile(
        "",
        "助詞\n",
        "000001,1,0,1,0,0,0,(),京都,,\n000001,1,0,2,0,0,1,(),Kyoto,,\n",
    )
    .unwrap();
    let mut analyzer = analyzer(resources, policy());
    let texts = analyzer.analyze("京都に").unwrap();
    assert_eq!(texts[0], "京都に");
    assert!(texts.iter().any(|t| t == "キョウト"));
    assert!(texts.iter().any(|t| t == "kyouto"));
    assert!(texts.iter().any(|t| t == "Kyoto"));
    assert!(!texts.iter().any(|t| t == "に"));
}

#[test]
fn synonym_limit_counts_only_unique_texts_added_after_basic_forms() {
    let source = "000001,,,,,,,,京都,,\n000001,,,,,,,,キョウト,,\n000001,,,,,,,,kyouto,,\n000001,,,,,,,,Kyoto,,\n000002,,,,,,,,京都,,\n000002,,,,,,,,Kyoto,,\n";
    let resources = Resources::compile("", "", source).unwrap();
    let mut exact = policy();
    exact.max_synonym_texts = 1;

    let texts = analyzer(resources.clone(), exact.clone())
        .analyze("京都")
        .unwrap();
    assert_eq!(texts, ["京都", "キョウト", "kyouto", "Kyoto"]);

    let source = format!("{source}000002,,,,,,,,ancient capital,,\n");
    let resources = Resources::compile("", "", &source).unwrap();
    let error = analyzer(resources.clone(), exact.clone())
        .analyze("京都")
        .unwrap_err();
    assert!(error.to_string().contains("synonym expansion limit"));

    exact.max_synonym_texts = 2;
    assert_eq!(
        analyzer(resources, exact).analyze("京都").unwrap(),
        ["京都", "キョウト", "kyouto", "Kyoto", "ancient capital"]
    );
}

#[test]
fn stopwords_remove_generated_forms_but_never_remove_the_original() {
    let mut analyzer = analyzer(Resources::compile("京都", "", "").unwrap(), policy());
    assert_eq!(analyzer.analyze("京都").unwrap(), ["京都"]);
}

#[test]
fn stoptags_filter_even_a_part_of_speech_allowed_by_the_policy() {
    let resources = Resources::compile("", "名詞\n", "").unwrap();
    let mut native = analyzer(resources, policy());
    assert_eq!(native.analyze("京都").unwrap(), ["京都"]);
}

#[test]
fn minus_one_disables_soft_limits_while_asix_text_capacity_remains_enforced() {
    let mut source = "000001,,,,,,,,京都,,\n".to_owned();
    for i in 0..300 {
        source.push_str(&format!("000001,,,,,,,,alias{i},,\n"));
    }
    let resources = Resources::compile("", "", &source).unwrap();
    assert!(
        analyzer(resources.clone(), policy())
            .analyze("京都")
            .is_err()
    );

    let mut unlimited = policy();
    unlimited.max_synonym_texts = -1;
    unlimited.max_document_units = -1;
    let mut native = analyzer(resources, unlimited);
    assert!(native.analyze("京都").unwrap().len() > 300);
    assert!(native.analyze(&"a".repeat(65536)).is_err());
}

#[test]
fn positive_document_limits_fail_explicitly_and_invalid_limits_are_rejected() {
    let mut small = policy();
    small.max_document_units = 1;
    assert!(
        analyzer(Resources::compile("", "", "").unwrap(), small)
            .analyze("京都")
            .is_err()
    );
    for invalid in [-2, 0] {
        let mut p = policy();
        p.max_synonym_texts = invalid;
        assert!(p.validate().is_err());
        p = policy();
        p.max_document_units = invalid;
        assert!(p.validate().is_err());
    }
}

#[test]
fn unlimited_policy_preserves_source_byte_limits_and_recovers_after_rejection() {
    let mut unlimited = policy();
    unlimited.max_synonym_texts = -1;
    unlimited.max_document_units = -1;
    unlimited.keep_part_of_speech = vec!["形容詞".into()];
    let mut native = analyzer(Resources::compile("", "", "").unwrap(), unlimited);

    for text in [
        "a".repeat(MAX_SOURCE_BYTES),
        "あ".repeat(MAX_SOURCE_BYTES / 3),
    ] {
        assert_eq!(native.analyze(&text).unwrap(), [text]);
    }

    for text in [
        "a".repeat(MAX_SOURCE_BYTES + 1),
        "あ".repeat(MAX_SOURCE_BYTES / 3 + 1),
    ] {
        assert!(text.encode_utf16().count() < 65535);
        let error = native.analyze(&text).unwrap_err().to_string();
        assert!(error.contains("Sudachi input capacity"), "{error}");
        assert_eq!(native.analyze("京都").unwrap(), ["京都"]);
    }
}

#[test]
fn sudachi_rewritten_byte_limit_is_independent_of_source_and_asix_limits() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/system.dic");
    let mut config = Config::minimal_at(PathResolver::from_embedded()).with_system_dic(path);
    config.input_text_plugins = Config::new_embedded()
        .unwrap()
        .input_text_plugins
        .into_iter()
        .filter(|plugin| plugin["class"] == "com.worksap.nlp.sudachi.DefaultInputTextPlugin")
        .collect();
    let mut unlimited = policy();
    unlimited.max_synonym_texts = -1;
    unlimited.max_document_units = -1;
    unlimited.keep_part_of_speech = vec!["形容詞".into()];
    let mut native =
        Analyzer::from_config(config, Resources::compile("", "", "").unwrap(), unlimited).unwrap();

    // NFKC expands each ㍿ into 株式会社 (12 UTF-8 bytes).
    let exact = format!("{}aaa", "㍿".repeat(5461));
    assert_eq!(
        native.analyze(&exact).unwrap().as_slice(),
        std::slice::from_ref(&exact)
    );
    let over = format!("{exact}a");
    assert!(over.len() < MAX_SOURCE_BYTES);
    let error = native.analyze(&over).unwrap_err().to_string();
    assert!(
        error.contains("65535") && error.contains("65536"),
        "{error}"
    );
    assert_eq!(native.analyze("京都").unwrap(), ["京都"]);
}
