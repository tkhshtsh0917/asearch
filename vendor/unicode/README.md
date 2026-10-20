# Unicode data for ja-basic-v1

The generated kana tables use Unicode 17.0.0's [UnicodeData.txt](https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt), SHA-256 `2e1efc1dcb59c575eedf5ccae60f95229f706ee6d031835247d843c11d96470c`.

Only U+FF61–FF9F's narrow decompositions and kana canonical compositions with U+3099/U+309A are extracted. The complete UCD is not bundled or downloaded during builds. Runtime normalization does not depend on Python, the browser's ICU or a general NFKC library.

Regenerate from an independently downloaded copy of that exact file:

```sh
python3 tools/unicode/generate.py UnicodeData.txt > crates/asearch-rs/src/normalize/tables.rs
```

The generator checks the source hash and uses the repository's pinned rustfmt. [Unicode License V3](LICENSE.txt) is retained verbatim from the [official license](https://www.unicode.org/license.txt), SHA-256 `e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96`. A copy is shipped with the Rust crate and the generated WASM package. This license covers the data-derived tables; new implementation code retains the project license.
