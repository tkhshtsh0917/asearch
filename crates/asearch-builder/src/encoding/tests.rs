//! Independent checks for count ceilings, size calculation and invalid settings.

use super::*;

fn rows() -> [Row<'static>; 2] {
    [1, 2].map(|key| Row {
        normalized: &[97, 98],
        original: Some(&[97, 98]),
        key,
        order: key - 1,
    })
}

fn settings(limits: ResourceLimits) -> Encoding {
    Encoding {
        options: Default::default(),
        has_original: true,
        limits,
        live_bytes: 0,
    }
}

#[test]
fn public_encoders_reject_count_and_unit_overflow() {
    for limits in [
        ResourceLimits {
            max_texts: 1,
            ..Default::default()
        },
        ResourceLimits {
            max_units: 3,
            ..Default::default()
        },
    ] {
        let settings = settings(limits);

        assert!(encode(rows().into_iter(), &[], settings).is_err());
        assert!(encoded_size(rows().into_iter(), &[], settings).is_err());
    }
}

#[test]
fn exact_limits_preserve_the_calculated_size() {
    let exact = settings(ResourceLimits {
        max_texts: 2,
        max_units: 4,
        ..Default::default()
    });

    let bytes = encode(rows().into_iter(), &[], exact).unwrap();
    let size = encoded_size(rows().into_iter(), &[], exact).unwrap();

    assert_eq!(bytes.len(), size);
}

#[test]
fn public_encoders_reject_invalid_settings() {
    let invalid_limits = settings(ResourceLimits {
        max_texts: 0,
        ..Default::default()
    });
    let invalid_options = Encoding {
        options: TransposedOptions {
            block_words: 0,
            ..Default::default()
        },
        ..settings(Default::default())
    };

    for settings in [invalid_limits, invalid_options] {
        assert!(encode(rows().into_iter(), &[], settings).is_err());
        assert!(encoded_size(rows().into_iter(), &[], settings).is_err());
    }
}
