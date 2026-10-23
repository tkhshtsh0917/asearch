use super::*;
use crate::{AmbigPolicy, Normalizer, Profile, QueryOptions};

/// The internal normalized-input path preserves the descriptor and matches
/// public raw-input construction for every supported normalization setting.
#[test]
fn normalized_input_preserves_symbols_and_query_descriptors() {
    let data: Vec<Vec<u16>> = ["ＡＢＣ　ｶﾞｰ", "ヴぁー", "😀AbＣ", "", "a b"]
        .into_iter()
        .map(|s| s.encode_utf16().collect())
        .chain([vec![0xd800, 0x41, 0xdc00]])
        .collect();
    let raw: Vec<_> = data.iter().map(Vec::as_slice).collect();
    let limits = ResourceLimits {
        max_pattern_units: 64,
        ..ResourceLimits::default()
    };

    for normalizer in (0..64)
        .map(|flags| Normalizer::from_descriptor(1, 1, flags).unwrap())
        .chain([Normalizer::NONE])
    {
        let normalized: Vec<_> = data
            .iter()
            .map(|text| {
                let mut out = Vec::with_capacity(text.len());
                normalizer.normalize_into(text, &mut out).unwrap();
                out
            })
            .collect();
        let refs: Vec<_> = normalized.iter().map(Vec::as_slice).collect();

        for profile in [Profile::Ideal, Profile::AsearchTs, Profile::AsearchTsLib] {
            let options = TransposedOptions {
                profile,
                normalizer,
                block_words: 1,
            };
            let public = TransposedEngine::new(&raw, options, limits).unwrap();
            let internal = TransposedEngine::from_normalized(&refs, options, limits).unwrap();
            let mut query = QueryWorkspace::new(
                QueryOptions {
                    profile,
                    normalizer,
                    ..QueryOptions::default()
                },
                limits,
            )
            .unwrap();
            let mut a = public.workspace(&query, 3).unwrap();
            let mut b = internal.workspace(&query, 3).unwrap();
            for k in 0..=3 {
                for text in &data {
                    query
                        .update_with_options(
                            text,
                            QueryOptions {
                                profile,
                                normalizer,
                                policy: AmbigPolicy::fixed(k).unwrap(),
                                ..QueryOptions::default()
                            },
                        )
                        .unwrap();
                    assert_eq!(
                        public.search(&query, &mut a).unwrap(),
                        internal.search(&query, &mut b).unwrap(),
                        "normalizer={normalizer:?}, profile={profile:?}, k={k}"
                    );
                }
            }
        }
    }
}
