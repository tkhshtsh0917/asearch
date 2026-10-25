# Asearch contributor instructions

- This repository owns Rust matching, CLI, WASM exports, Rust tests and fuzz targets. JS/TS APIs, bindings generation, browser verification and benchmarks belong to asearch-demo/integration.
- Run cargo fmt --all --check, cargo clippy --locked --workspace --all-targets -- -D warnings and cargo test --locked --workspace.
- Preserve profiles, binary format, resource limits and atomic update semantics. Do not change behavior during structural edits.
- Doc comments describe responsibilities, behavior, inputs, outputs and essential safety constraints. Put provenance in README acknowledgments; retain legal notices.
- Do not expose secrets, personal paths or private environment dependencies.

- README.md and docs outside ja/ are English sources; README.ja.md and docs/ja are faithful translations updated in the same change, preserving examples, constraints and dated evidence.

- Follow CONTRIBUTING.md for development checks and documentation responsibilities.
