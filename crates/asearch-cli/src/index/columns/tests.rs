//! Publication identity, custom descriptor and canonical compaction regressions.

use super::*;

#[test]
fn sri_encoding_and_manifest_identity_validation() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert!(manifest("bad id", "1", &[]).is_err());
    assert!(manifest("faq", "9007199254740992", &[]).is_err());
    assert!(manifest("faq", "1", &["../private.asix=missing".into()]).is_err());
}

#[test]
fn custom_descriptors_survive_manifest_and_compaction() {
    struct Temporary(std::path::PathBuf);

    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = Temporary(
        std::env::temp_dir().join(format!("asearch-manifest-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).unwrap();
    let text: Vec<u16> = "Ａ😀".encode_utf16().collect();
    let refs = [text.as_slice()];
    let docs = [asearch_engine::Doc {
        key: 1,
        order: 0,
        texts: &refs,
    }];
    for profile in [
        asearch_engine::Profile::Ideal,
        asearch_engine::Profile::AsearchTsLib,
    ] {
        for normalizer in [
            asearch_engine::Normalizer::NONE,
            asearch_engine::Normalizer::JA_BASIC,
        ] {
            let options = TransposedOptions {
                profile,
                normalizer,
                ..Default::default()
            };
            let bytes =
                asearch_builder::build(&docs, &[], options, ResourceLimits::default()).unwrap();
            let path = directory.0.join("base.asix");
            std::fs::write(&path, &bytes).unwrap();
            let local = path.to_str().unwrap();
            assert_eq!(compact(&[local]).unwrap(), bytes);
            let data: Value = serde_json::from_slice(
                &manifest("test", "1", &[format!("base.asix={local}")]).unwrap(),
            )
            .unwrap();
            assert_eq!(
                data["index"]["symbol_unit"],
                if profile == asearch_engine::Profile::AsearchTsLib {
                    "utf16"
                } else {
                    "code-point"
                }
            );
            assert_eq!(
                data["index"]["normalizer"]["id"],
                if normalizer == asearch_engine::Normalizer::NONE {
                    "none"
                } else {
                    "ja-basic"
                }
            );
            assert_eq!(
                data["index"]["normalizer"]["options"],
                normalizer.descriptor().2
            );
        }
    }
}
