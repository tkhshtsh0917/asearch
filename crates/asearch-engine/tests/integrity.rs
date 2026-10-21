use asearch_engine::{AsearchIndex, Doc, ExpectedSegment, ResourceLimits, TransposedOptions};
use sha2::{Digest, Sha384};

#[test]
fn trusted_descriptors_are_verified_before_candidate_construction() {
    let text: Vec<_> = "ｶﾞと検索".encode_utf16().collect();
    let texts = [text.as_slice()];
    let options = TransposedOptions::default();
    let limits = ResourceLimits::default();
    let index = AsearchIndex::from_docs(
        &[Doc {
            key: 3,
            order: 1,
            texts: &texts,
        }],
        options,
        limits,
    )
    .unwrap();
    let bytes = index.to_bytes().unwrap();
    let expected = ExpectedSegment {
        id: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
        bytes: bytes.len(),
        texts: 1,
        sha384: Sha384::digest(&bytes).into(),
    };
    let loaded = AsearchIndex::from_bytes_expected(&bytes, options, limits, &expected).unwrap();
    assert_eq!(loaded.to_bytes().unwrap(), bytes);
    assert!(
        AsearchIndex::from_owned_bytes_expected(bytes.clone(), options, limits, &expected).is_ok()
    );
    for bad in [
        ExpectedSegment {
            bytes: expected.bytes + 1,
            ..expected
        },
        ExpectedSegment {
            texts: 2,
            ..expected
        },
        ExpectedSegment {
            id: expected.id ^ 1,
            ..expected
        },
        ExpectedSegment {
            sha384: [0; 48],
            ..expected
        },
    ] {
        assert!(AsearchIndex::from_bytes_expected(&bytes, options, limits, &bad).is_err());
    }
    let mut changed = bytes.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert!(expected.verify(&changed).is_err());
    assert!(
        AsearchIndex::from_owned_bytes_expected(
            bytes,
            options,
            ResourceLimits {
                max_total_bytes: 64,
                ..limits
            },
            &expected
        )
        .is_err()
    );
}
