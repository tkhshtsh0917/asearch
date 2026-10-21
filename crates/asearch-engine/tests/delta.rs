use asearch_engine::{AsearchIndex, Doc, ResourceLimits, TransposedOptions};
use xxhash_rust::xxh3::Xxh3;

fn repair(bytes: &mut [u8]) {
    let mut hash = Xxh3::new();
    hash.update(&bytes[..24]);
    hash.update(&[0; 8]);
    hash.update(&bytes[32..]);
    bytes[24..32].copy_from_slice(&hash.digest().to_le_bytes());
}
fn tomb_offset(bytes: &[u8]) -> usize {
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    (0..count)
        .find_map(|i| {
            let at = 64 + 16 * i;
            (&bytes[at..at + 4] == b"TOMB")
                .then(|| u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize)
        })
        .unwrap()
}
#[test]
fn canonical_tombstones_roundtrip_with_same_segment_replacement_and_removal_only() {
    let text: Vec<_> = "ｶﾞと新しい本文".encode_utf16().collect();
    let texts = [text.as_slice()];
    let docs = [Doc {
        key: 2,
        order: 3,
        texts: &texts,
    }];
    let options = TransposedOptions::default();
    let limits = ResourceLimits::default();
    let index = AsearchIndex::from_delta(&docs, &[9, 2, 1], options, limits).unwrap();
    assert_eq!(index.tombstones(), &[1, 2, 9]);
    assert_eq!(index.original_text(0), Some(text.as_slice()));
    let bytes = index.to_bytes().unwrap();
    let mut without_originals = bytes.clone();
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = 64 + i * 16;
        if &without_originals[at..at + 4] == b"OLEN" {
            without_originals[at..at + 4].copy_from_slice(b"ZLEN");
        } else if &without_originals[at..at + 4] == b"OTXT" {
            without_originals[at..at + 4].copy_from_slice(b"ZTXT");
        }
    }
    repair(&mut without_originals);
    let normalized_only = AsearchIndex::from_bytes(&without_originals, options, limits).unwrap();
    assert_eq!(normalized_only.original_text(0), None);
    assert_eq!(normalized_only.tombstones(), &[1, 2, 9]);
    let canonical_normalized = normalized_only.to_bytes().unwrap();
    assert_eq!(
        AsearchIndex::from_bytes(&canonical_normalized, options, limits)
            .unwrap()
            .to_bytes()
            .unwrap(),
        canonical_normalized
    );
    let loaded = AsearchIndex::from_bytes(&bytes, options, limits).unwrap();
    assert_eq!(loaded.tombstones(), &[1, 2, 9]);
    assert_eq!(loaded.to_bytes().unwrap(), bytes);
    assert_eq!(
        AsearchIndex::from_delta(&docs, &[1, 9, 2], options, limits)
            .unwrap()
            .to_bytes()
            .unwrap(),
        bytes
    );
    let empty = AsearchIndex::from_delta(&[], &[1, 2], options, limits).unwrap();
    let removed =
        AsearchIndex::from_owned_bytes(empty.to_bytes().unwrap(), options, limits).unwrap();
    assert_eq!(removed.text_count(), 0);
    assert_eq!(removed.tombstones(), &[1, 2]);
    assert!(AsearchIndex::from_delta(&docs, &[1, 1], options, limits).is_err());
    assert!(
        AsearchIndex::from_delta(
            &[],
            &[1; 20],
            options,
            ResourceLimits {
                max_total_bytes: 64,
                ..limits
            }
        )
        .is_err()
    );
    let base = AsearchIndex::from_docs(&docs, options, limits)
        .unwrap()
        .to_bytes()
        .unwrap();
    assert_eq!(
        AsearchIndex::from_delta(&docs, &[], options, limits)
            .unwrap()
            .to_bytes()
            .unwrap(),
        base
    );
}
#[test]
fn malformed_tombstone_order_duplicates_counts_and_flags_are_rejected() {
    let options = TransposedOptions::default();
    let limits = ResourceLimits::default();
    let bytes = AsearchIndex::from_delta(&[], &[1, 2], options, limits)
        .unwrap()
        .to_bytes()
        .unwrap();
    let offset = tomb_offset(&bytes);
    for value in [0u32, 1] {
        let mut bad = bytes.clone();
        bad[offset + 4..offset + 8].copy_from_slice(&value.to_le_bytes());
        repair(&mut bad);
        assert!(AsearchIndex::from_bytes(&bad, options, limits).is_err());
    }
    for count in [0u32, 1, 3, u32::MAX] {
        let mut bad = bytes.clone();
        bad[44..48].copy_from_slice(&count.to_le_bytes());
        repair(&mut bad);
        assert!(AsearchIndex::from_bytes(&bad, options, limits).is_err());
    }
}
