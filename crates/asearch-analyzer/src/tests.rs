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
    assert_eq!(lookup.alternatives(&["入口"]), ["入口", "出口"]);
    assert_eq!(lookup.alternatives(&["削除済み"]), Vec::<&str>::new());
    assert_eq!(lookup.alternatives(&["出口"]), ["出口", "別グループ"]);
}

#[test]
fn csv_quoting_omitted_flags_and_case_insensitive_triggers_are_supported() {
    let source = "000001,,,,,,,,\"A,B\",,\n000001,,,,,,,,alpha,,\n";
    let resources = Resources::compile("こと\n# comment\n", "助詞,格助詞\n", source).unwrap();
    assert_eq!(
        resources::Lookup::new(&resources).alternatives(&["ALPHA"]),
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
