#![no_main]

use asearch_rs::{AsearchIndex, Normalizer, Profile, ResourceLimits, TransposedOptions};
use libfuzzer_sys::fuzz_target;
use xxhash_rust::xxh3::xxh3_64;

fn exercise(data: &[u8], options: TransposedOptions) {
    let limits = ResourceLimits {
        max_total_bytes: 4 * 1024 * 1024,
        max_working_bytes: 8 * 1024 * 1024,
        max_texts: 1024,
        max_units: 16_384,
        max_slots: 131_072,
        max_pattern_units: 16,
        ..ResourceLimits::default()
    };
    if let Ok(index) = AsearchIndex::from_bytes(data, options, limits) {
        let canonical = index.to_bytes().expect("bounded valid index serializes");
        let restored =
            AsearchIndex::from_bytes(&canonical, options, limits).expect("canonical index reloads");
        assert_eq!(restored.to_bytes().unwrap(), canonical);
        assert_eq!(index.document_count(), restored.document_count());
        assert_eq!(index.text_count(), restored.text_count());
        assert_eq!(index.tombstones(), restored.tombstones());
        for i in 0..index.text_count() {
            assert_eq!(index.text(i), restored.text(i));
            assert_eq!(index.original_text(i), restored.original_text(i));
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 65_536 {
        return;
    }
    let base = TransposedOptions {
        block_words: 1,
        ..TransposedOptions::default()
    };
    exercise(data, base);
    if data.len() < 64 || data[32] > 1 {
        return;
    }
    let normalizer = Normalizer::from_descriptor(
        u16::from_le_bytes([data[48], data[49]]),
        u16::from_le_bytes([data[50], data[51]]),
        u32::from_le_bytes([data[52], data[53], data[54], data[55]]),
    );
    if let Ok(normalizer) = normalizer {
        let mut repaired = data.to_vec();
        // Also reach structural validation behind the checksum/length gates.
        // The raw path above still exercises malformed checksum and lengths.
        repaired[16..24].copy_from_slice(&(data.len() as u64).to_le_bytes());
        repaired[24..32].fill(0);
        let hash = xxh3_64(&repaired);
        repaired[24..32].copy_from_slice(&hash.to_le_bytes());
        exercise(
            &repaired,
            TransposedOptions {
                normalizer,
                profile: if data[32] == 1 {
                    Profile::AsearchTsLib
                } else {
                    Profile::Ideal
                },
                ..base
            },
        );
    }
});
