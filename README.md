# asearch

[日本語](README.ja.md)

A Rust library for approximate matching with wildcards. It provides bulk indexes, Japanese normalization, incremental search, atomic base/delta updates and DAG-based intent expressions.

## Layout

- `crates/asearch-index`: ASIX v1 definitions, validation and shared normalization.
- `crates/asearch-builder`: Server-side base/delta construction and canonical serialization.
- `crates/asearch-analyzer`: Server-only Sudachi analysis, filtering, synonyms and romanization.
- `crates/asearch-engine`: Loading, derived search structures, matching, update adoption and explanations.
- `crates/asearch-cli`: Index building, search, delta generation, compaction and manifests.
- `crates/asearch-wasm`: The Rust boundary exposing the core through WebAssembly.
- `fuzz`: Validation of index and intent-expression inputs.

[asearch-demo](https://github.com/tkhshtsh0917/asearch-demo) owns the TS/JS adapter, bindings generation, browser tests and search demo. Building this repository does not require Node.js.

## Design documentation

- [Architecture](docs/architecture.md): SVG diagrams of crate cooperation across server, CDN and browser.
- [Bitap walkthrough](docs/algorithm.md): GIF animations and numerical examples of bit operations.
- [ASIX v1 byte layout](docs/asix-format.md): Every byte offset, layout diagrams and a CLI-reproducible example.
- [Native analysis](docs/analysis.md): Server-only language analysis, resource conversion and disabling optional limits with `-1`.
- [Result ranking](docs/ranking.md): Stable ordering and opt-in ranking by edit distance and match position (0.3+).

The [documentation index](docs/README.md) links to English sources and Japanese translations.

## Build

Install Git and rustup, then run these commands at the repository root. `rust-toolchain.toml` pins the Rust version and WASM target.

```sh
cargo build --locked --workspace
cargo build --locked -p asearch-wasm --target wasm32-unknown-unknown --release
cargo doc --workspace --no-deps
```

WASM supports reading and searching only and does not depend on asearch-builder. Use the CLI or asearch-builder for generation and saving. The native engine's optional, default-enabled `builder` feature preserves Rust serialization conveniences.

The WASM output is `target/wasm32-unknown-unknown/release/asearch_wasm.wasm`. Follow the demo repository's instructions for browser glue generation, optimization and JS APIs.

## Rust usage

```rust
use asearch_engine::{Asearch, MatchMode};
fn main() -> Result<(), asearch_engine::PatternTooLong> {
    let pattern = Asearch::new("hello", MatchMode::Exact)?;
    assert!(pattern.is_match("helo", 1));
    Ok(())
}
```

Use `crates/asearch-engine` as a path dependency. `cargo doc` documents the index and workspace APIs.

```sh
cargo run --locked -p asearch-cli -- build docs.jsonl questions.asix
cargo run --locked -p asearch-cli -- search questions.asix 検索語
cargo run --locked -p asearch-cli -- delta changes.jsonl delta.asix
cargo run --locked -p asearch-cli -- compact compact.asix questions.asix delta.asix
cargo run --locked -p asearch-cli -- manifest faq 1 manifest.json base.asix=questions.asix
```

Build input is JSONL, one document per line, such as `{"key":1,"texts":["利用方法"]}`. Delta input is `{"op":"upsert","key":1,"texts":["新しい内容"]}` or `{"op":"remove","key":1}`. Output files are newly created. The host owns delivery and generation management.

## Acknowledgments and compatibility

Inspired by [asearch.ts](https://github.com/teramotodaiki/asearch.ts), with compatibility testing against 1.0.3 as the starting point. `AsearchTs` matches code points, `AsearchTsLib` matches UTF-16 units, and `Ideal` provides monotonic edit-distance semantics. Acceptance at individual levels need not be monotonic in compatibility profiles. Specifications and compatibility oracles live in [the demo integration workspace](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration).

## License

New code is MIT OR Apache-2.0. See [NOTICE](NOTICE) and each crate's licenses.

See [Contributing](CONTRIBUTING.md) for development checks and change guidelines.

## Release Assets

Download distributions for Linux x86_64, macOS aarch64 and Windows x86_64 from [GitHub Releases](https://github.com/tkhshtsh0917/asearch/releases).

- `asearch_wasm.wasm`: Baseline-instruction, read-only search WASM.
- `asearch-cli-PLATFORM`: Native CLI; Windows uses `.exe`.
- `asearch-assets-PLATFORM.tar.gz`: Bundled WASM, CLI and licenses.
- `asearch-licenses.tar.gz`: Licenses and third-party notices.
- `SHA256SUMS`: SHA-256 checksums for Release Assets.

Pin a tag and compare downloaded files with its checksums before use. Checksums detect mismatches; publisher authentication is a separate concern. For JS/TS usage, see [the demo integration workspace](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration).

See [performance decisions](docs/performance.md) for measured optimization scopes and build tradeoffs.
