use asearch_rs::{NormalizeError, Normalizer};

fn normalize(normalizer: Normalizer, text: &str) -> String {
    let input: Vec<_> = text.encode_utf16().collect();
    let mut output = Vec::with_capacity(input.len());
    normalizer.normalize_into(&input, &mut output).unwrap();
    String::from_utf16(&output).unwrap()
}

#[test]
fn default_and_individual_transformations() {
    assert_eq!(
        normalize(Normalizer::default(), "　ＡＢＣｶﾞﾊﾟｳﾞﾜﾞｦﾞカﾞかﾞ"),
        " abcがぱゔヷヺがが"
    );
    for (flags, input, expected) in [
        (0, "　ＡｶﾞカA－", "　ＡｶﾞカA－"),
        (1, "　ＡｶﾞカA－", " ＡｶﾞカA－"),
        (2, "　ＡｶﾞカA－", "　AｶﾞカA-"),
        (4, "ｶﾞﾊﾟｳﾞﾜﾞｦﾞ", "ガパヴヷヺ"),
        (8, "ガパヴヷヺ", "がぱゔヷヺ"),
        (16, "ＡABC", "Ａabc"),
        (32, "ーｰ－‐―−-", "ーーーーーーー"),
        (63, "　ＡｶﾞカA－", " aがかaー"),
    ] {
        let normalizer = Normalizer::from_descriptor(1, 1, flags).unwrap();
        assert_eq!(normalize(normalizer, input), expected, "flags={flags}");
    }
    let text = "　ＡＢＣｶﾞ😀";
    assert_eq!(normalize(Normalizer::NONE, text), text);
    assert_eq!(normalize(Normalizer::default(), "①Åﬃé"), "①Åﬃé");
}

#[test]
fn descriptors_and_capacity_errors_are_explicit_and_preserve_output() {
    assert_eq!(Normalizer::NONE.descriptor(), (0, 1, 0));
    assert_eq!(Normalizer::JA_BASIC.descriptor(), (1, 1, 31));
    assert_eq!(
        Normalizer::from_descriptor(2, 1, 0),
        Err(NormalizeError::UnsupportedId)
    );
    assert_eq!(
        Normalizer::from_descriptor(1, 2, 0),
        Err(NormalizeError::UnsupportedVersion)
    );
    assert_eq!(
        Normalizer::from_descriptor(0, 1, 1),
        Err(NormalizeError::InvalidOptions)
    );
    for flags in [64, u32::MAX] {
        assert_eq!(
            Normalizer::from_descriptor(1, 1, flags),
            Err(NormalizeError::InvalidOptions)
        );
    }
    let mut output = vec![42];
    let input = vec![0xff76; output.capacity() + 1];
    assert_eq!(
        Normalizer::JA_BASIC.normalize_into(&input, &mut output),
        Err(NormalizeError::NeedsCapacity {
            required: input.len()
        })
    );
    assert_eq!(output, [42]);
    Normalizer::NONE.normalize_into(&[], &mut output).unwrap();
    assert!(output.is_empty());
}

#[test]
fn all_units_and_kana_mark_pairs_are_idempotent_for_every_option_set() {
    let mut once = Vec::with_capacity(2);
    let mut twice = Vec::with_capacity(2);
    for options in 0..64 {
        let normalizer = Normalizer::from_descriptor(1, 1, options).unwrap();
        for unit in 0..=u16::MAX {
            normalizer.normalize_into(&[unit], &mut once).unwrap();
            normalizer.normalize_into(&once, &mut twice).unwrap();
            assert_eq!(once, twice, "unit={unit:x} options={options}");
        }
        for base in (0x3040..=0x30ff).chain(0xff61..=0xff9f) {
            for mark in [0xff9e, 0xff9f, 0x3099, 0x309a] {
                normalizer.normalize_into(&[base, mark], &mut once).unwrap();
                normalizer.normalize_into(&once, &mut twice).unwrap();
                assert_eq!(once, twice, "base={base:x} mark={mark:x} options={options}");
            }
        }
    }
}

#[test]
fn mixed_sequences_preserve_surrogates_and_are_idempotent() {
    let alphabet = [
        0xff76, 0xff9e, 0xff9f, 0x30ef, 0x30f2, 0x304b, 0x30ab, 0x3099, 0xff21, 0xff0d, 0x3000,
        0x2015, 0xd800, 0xdc00,
    ];
    let mut seed = 31u64;
    let mut input = [0; 32];
    let mut once = Vec::with_capacity(32);
    let mut twice = Vec::with_capacity(32);
    for _ in 0..1000 {
        for unit in &mut input {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *unit = alphabet[(seed >> 32) as usize % alphabet.len()];
        }
        for options in 0..64 {
            let normalizer = Normalizer::from_descriptor(1, 1, options).unwrap();
            normalizer.normalize_into(&input, &mut once).unwrap();
            normalizer.normalize_into(&once, &mut twice).unwrap();
            assert_eq!(once, twice);
        }
    }
    let input = [0xd83d, 0xde00, 0xd800, 0xdc01, 0xdc00];
    Normalizer::JA_BASIC
        .normalize_into(&input, &mut once)
        .unwrap();
    assert_eq!(once, input);
}
