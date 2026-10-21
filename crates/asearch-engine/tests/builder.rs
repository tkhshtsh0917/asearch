use asearch_engine::{AsearchIndex, Doc, Normalizer, Profile, ResourceLimits, TransposedOptions};

#[test]
fn server_builder_preserves_v1_golden_bytes_and_runtime_loading() {
    let texts: Vec<Vec<u16>> = ["ＡＢＣ ｶﾞ", "😀", "", "検索です"]
        .iter()
        .map(|s| s.encode_utf16().collect())
        .collect();
    let refs: Vec<_> = texts.iter().map(Vec::as_slice).collect();
    let docs = [
        Doc {
            key: 7,
            order: 2,
            texts: &refs[..3],
        },
        Doc {
            key: 2,
            order: 1,
            texts: &refs[3..],
        },
    ];
    let limits = ResourceLimits::default();
    for profile in [Profile::Ideal, Profile::AsearchTs, Profile::AsearchTsLib] {
        for flags in 0..64 {
            let options = TransposedOptions {
                profile,
                normalizer: Normalizer::from_descriptor(1, 1, flags).unwrap(),
                ..Default::default()
            };
            for tombs in [&[][..], &[2, 9][..]] {
                let bytes = asearch_builder::build(&docs, tombs, options, limits).unwrap();
                let native = AsearchIndex::from_delta(&docs, tombs, options, limits).unwrap();
                assert_eq!(bytes, native.to_bytes().unwrap());
                assert_eq!(
                    bytes,
                    AsearchIndex::from_bytes(&bytes, options, limits)
                        .unwrap()
                        .to_bytes()
                        .unwrap()
                );
            }
        }
    }
    assert_eq!(
        asearch_builder::build(&docs, &[], Default::default(), limits).unwrap(),
        include_bytes!("fixtures/canonical-v1.asix")
    );
    assert!(asearch_builder::build(&docs, &[2, 2], Default::default(), limits).is_err());
    assert!(asearch_builder::build(&[docs[0], docs[0]], &[], Default::default(), limits).is_err());
}
