use asearch_rs::{
    AmbigPolicy, AsearchIndex, Doc, MatchMode, Normalizer, QueryOptions, QueryWorkspace,
    ResourceLimits, TransposedOptions,
};
use xxhash_rust::xxh3::xxh3_64;

fn options() -> TransposedOptions {
    TransposedOptions {
        normalizer: Normalizer::NONE,
        block_words: 1,
        ..TransposedOptions::default()
    }
}

fn repair(bytes: &mut [u8]) {
    bytes[24..32].fill(0);
    let hash = xxh3_64(bytes);
    bytes[24..32].copy_from_slice(&hash.to_le_bytes());
}

/// An independent small wire encoder avoids testing only the writer's output.
fn wire(sections: &[([u8; 4], u32, Vec<u8>)]) -> Vec<u8> {
    let header = 64 + sections.len() * 16;
    let mut bytes = vec![0; header];
    bytes[..4].copy_from_slice(b"ASIX");
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    bytes[8..12].copy_from_slice(&(header as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(sections.len() as u32).to_le_bytes());
    bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&1u32.to_le_bytes());
    bytes[50..52].copy_from_slice(&1u16.to_le_bytes());
    for (i, (tag, flags, data)) in sections.iter().enumerate() {
        let start = bytes.len();
        bytes.extend_from_slice(data);
        bytes.resize(bytes.len().div_ceil(16) * 16, 0);
        let slot = 64 + i * 16;
        bytes[slot..slot + 4].copy_from_slice(tag);
        bytes[slot + 4..slot + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[slot + 8..slot + 12].copy_from_slice(&(start as u32).to_le_bytes());
        bytes[slot + 12..slot + 16].copy_from_slice(&(data.len() as u32).to_le_bytes());
    }
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    repair(&mut bytes);
    bytes
}

fn required() -> Vec<([u8; 4], u32, Vec<u8>)> {
    vec![
        (*b"TEXT", 1, vec![b'a', 0]),
        (*b"TLEN", 1, vec![1, 0]),
        (*b"KEYS", 1, 42u32.to_le_bytes().to_vec()),
    ]
}

#[test]
fn roundtrip_is_canonical_across_input_order_and_runtime_layout() {
    let raw: Vec<_> = "ｶﾞＡ😀".encode_utf16().collect();
    let raw2 = [0xd800, 0x61];
    let first = [raw.as_slice()];
    let second = [raw2.as_slice(), &[]];
    let docs = [
        Doc {
            key: 10,
            order: 5,
            texts: &first,
        },
        Doc {
            key: 2,
            order: 1,
            texts: &second,
        },
    ];
    let limits = ResourceLimits::default();
    let index = AsearchIndex::from_docs(
        &docs,
        TransposedOptions {
            block_words: 1,
            ..TransposedOptions::default()
        },
        limits,
    )
    .unwrap();
    let bytes = index.to_bytes().unwrap();
    let reversed =
        AsearchIndex::from_docs(&[docs[1], docs[0]], TransposedOptions::default(), limits).unwrap();
    assert_eq!(bytes, reversed.to_bytes().unwrap());
    let loaded = AsearchIndex::from_bytes(&bytes, TransposedOptions::default(), limits).unwrap();
    assert_eq!(loaded.to_bytes().unwrap(), bytes);
    for i in 0..3 {
        assert_eq!(loaded.text(i), index.text(i));
        assert_eq!(loaded.original_text(i), index.original_text(i));
    }
    let mut query = QueryWorkspace::new(
        QueryOptions {
            mode: MatchMode::Include,
            policy: AmbigPolicy::fixed(2).unwrap(),
            ..QueryOptions::default()
        },
        limits,
    )
    .unwrap();
    query
        .update(&"がa".encode_utf16().collect::<Vec<_>>())
        .unwrap();
    let mut a = index.workspace(&query, 7).unwrap();
    let mut b = loaded.workspace(&query, 7).unwrap();
    let before = index.search(&query, &mut a, 30, true).unwrap();
    let after = loaded.search(&query, &mut b, 30, true).unwrap();
    assert_eq!(before.hits, after.hits);
    assert_eq!(before.totals, after.totals);
    let empty = AsearchIndex::from_docs(&[], TransposedOptions::default(), limits)
        .unwrap()
        .to_bytes()
        .unwrap();
    assert!(AsearchIndex::from_bytes(&empty, TransposedOptions::default(), limits).is_ok());
}

#[test]
fn optional_originals_orders_and_unknown_sections_follow_wire_contract() {
    let limits = ResourceLimits::default();
    let mut sections = required();
    sections.push((*b"MORE", 0, vec![1, 2, 3]));
    let index = AsearchIndex::from_bytes(&wire(&sections), options(), limits).unwrap();
    assert_eq!(index.text(0), Some([0x61].as_slice()));
    assert!(index.original_text(0).is_none());
    let rewritten = index.to_bytes().unwrap();
    assert_eq!(u32::from_le_bytes(rewritten[12..16].try_into().unwrap()), 4);
    sections.last_mut().unwrap().1 = 1;
    assert!(AsearchIndex::from_bytes(&wire(&sections), options(), limits).is_err());
    sections.last_mut().unwrap().1 = 0;
    sections.push((*b"MORE", 0, vec![]));
    assert!(AsearchIndex::from_bytes(&wire(&sections), options(), limits).is_err());
}

#[test]
fn validation_checks_counts_padding_reservations_and_original_consistency() {
    let valid = wire(&required());
    let limits = ResourceLimits::default();
    for (offset, value) in [
        (4, 2),
        (8, 0),
        (12, 255),
        (32, 1),
        (33, 1),
        (34, 1),
        (36, 2),
        (40, 2),
        (44, 1),
        (48, 2),
        (50, 2),
        (52, 1),
        (56, 1),
        (68, 0),
        (72, 0),
        (76, 3),
        (114, 1),
    ] {
        let mut bytes = valid.clone();
        bytes[offset] = value;
        repair(&mut bytes);
        assert!(
            AsearchIndex::from_bytes(&bytes, options(), limits).is_err(),
            "offset={offset}"
        );
    }
    for end in 0..valid.len() {
        assert!(AsearchIndex::from_bytes(&valid[..end], options(), limits).is_err());
    }
    let mut sections = required();
    sections.push((*b"OLEN", 0, vec![1, 0]));
    sections.push((*b"OTXT", 0, vec![b'b', 0]));
    assert!(AsearchIndex::from_bytes(&wire(&sections), options(), limits).is_err());
    sections.last_mut().unwrap().2 = vec![b'a', 0];
    assert!(AsearchIndex::from_bytes(&wire(&sections), options(), limits).is_ok());
    assert!(
        AsearchIndex::from_bytes(
            &valid,
            options(),
            ResourceLimits {
                max_working_bytes: 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        AsearchIndex::from_bytes(
            &valid,
            options(),
            ResourceLimits {
                max_total_bytes: valid.len() - 1,
                ..limits
            }
        )
        .is_err()
    );
}

#[test]
fn deterministic_mutation_corpus_never_panics() {
    let base = wire(&required());
    let limits = ResourceLimits {
        max_working_bytes: 2 * 1024 * 1024,
        max_texts: 128,
        max_units: 1024,
        max_total_bytes: 4096,
        ..ResourceLimits::default()
    };
    let mut seed = 31_u64;
    for _ in 0..4000 {
        let mut bytes = base.clone();
        for _ in 0..3 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let i = (seed >> 32) as usize % bytes.len();
            bytes[i] ^= (seed as u8) | 1;
        }
        // Exercise structural validation beyond the checksum gate as well.
        if seed & 1 == 0 {
            repair(&mut bytes);
        }
        let _ = AsearchIndex::from_bytes(&bytes, options(), limits);
    }
}
