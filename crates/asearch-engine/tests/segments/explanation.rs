//! Original preservation and generation-safe explanations.

use super::helpers::*;

fn without_originals(index: AsearchIndex) -> AsearchIndex {
    use xxhash_rust::xxh3::Xxh3;
    let mut bytes = index.to_bytes().unwrap();
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = 64 + 16 * i;
        if &bytes[at..at + 4] == b"OLEN" {
            bytes[at..at + 4].copy_from_slice(b"ZLEN");
        } else if &bytes[at..at + 4] == b"OTXT" {
            bytes[at..at + 4].copy_from_slice(b"ZTXT");
        }
    }
    let mut h = Xxh3::new();
    h.update(&bytes[..24]);
    h.update(&[0; 8]);
    h.update(&bytes[32..]);
    bytes[24..32].copy_from_slice(&h.digest().to_le_bytes());
    AsearchIndex::from_bytes(
        &bytes,
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap()
}

#[test]
fn compaction_never_fabricates_or_discards_live_originals() {
    assert!(
        IndexSet::new(
            vec![
                segment(&[(1, 1, &["Ａ"])], &[]),
                without_originals(segment(&[(2, 2, &["Ｂ"])], &[]))
            ],
            limits()
        )
        .is_err()
    );
    let set = IndexSet::new(
        vec![
            segment(&[(1, 1, &["Ａ"])], &[]),
            without_originals(segment(&[(2, 2, &["Ｂ"])], &[1])),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(
        set.to_bytes().unwrap(),
        without_originals(segment(&[(2, 2, &["Ｂ"])], &[]))
            .to_bytes()
            .unwrap()
    );
    let removed = IndexSet::new(
        vec![
            without_originals(segment(&[(1, 1, &["Ａ"])], &[])),
            segment(&[], &[1]),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(removed.document_count(), 0);
    assert_eq!(
        removed.to_bytes().unwrap(),
        segment(&[], &[]).to_bytes().unwrap()
    );
}

#[test]
fn explanation_membership_checks_every_alias_and_rejects_dead_or_out_of_range_texts() {
    let set = IndexSet::new(parts(), limits()).unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            mode: MatchMode::Include,
            policy: AmbigPolicy::fixed(0).unwrap(),
            ..QueryOptions::default()
        },
        limits(),
    )
    .unwrap();
    query.update(&[]).unwrap();

    // Segment order differs from global document order; replacement masks
    // hide the old key while keeping every alias in the replacing segment.
    for (segment, live) in [
        (0, vec![false, true, true, false]),
        (1, vec![true, true, false]),
        (2, vec![true, true]),
    ] {
        for (text, expected) in live.iter().enumerate() {
            let explanation = set.explain(segment, text, &query, None, None);
            if *expected {
                assert_eq!(
                    explanation.unwrap().unwrap().text,
                    set.original_text(segment, text).unwrap()
                );
            } else {
                assert!(explanation.is_err(), "dead text {segment}/{text}");
            }
        }
        for text in [live.len(), usize::MAX] {
            assert!(set.explain(segment, text, &query, None, None).is_err());
        }
    }
    assert!(set.explain(3, 0, &query, None, None).is_err());

    let empty = IndexSet::new(vec![segment(&[], &[])], limits()).unwrap();
    assert!(empty.explain(0, 0, &query, None, None).is_err());
}
