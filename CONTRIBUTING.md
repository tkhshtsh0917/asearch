# Contributing

[日本語](CONTRIBUTING.ja.md)

## Scope

This repository owns Rust matching, ASIX validation and construction, native analysis, the CLI and WASM exports. The [demo integration workspace](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration) owns JS/TS bindings, browser tests and compatibility models. Start with the [README](README.md) and [architecture](docs/architecture.md).

## Development checks

Install rustup; `rust-toolchain.toml` selects the compiler, components and WASM target. From the repository root, run:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p asearch-wasm --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --workspace
cargo doc --workspace --no-deps
```

[Fuzzing](fuzz/README.md) uses a separate pinned nightly toolchain. Build instructions are in the README; product builds do not require Node.js.

## Changes and regressions

Keep each change focused, with functions and files separated by responsibility and blank lines between meaningful processing stages. Comments explain behavior and essential constraints. Keep product Rust free of unsafe code. Keep dependencies pinned and generated files out of Git. Preserve upstream licenses and notices, and exclude secrets and workstation-specific paths.

Add regression coverage for behavioral fixes. Use independent matching/alignment models when changing algorithms; another copy of the optimized implementation is not an independent oracle. Preserve ASIX v1, normalization descriptors, resource limits, complete candidate history, result lifetimes and atomic adoption unless a compatibility change is explicit. Cover failure recovery as well as successful inputs.

For performance changes, compare equivalent results before timing, prepare both artifacts with the same toolchain, and use paired runs plus a separate confirmation. Record workload/artifact/source identities and regressions; do not use noisy shared-CI timings as speed guarantees. See [performance decisions](docs/performance.md).

## Documentation

README files and specifications describe supported behavior and use. This guide and test/tool guides describe development and validation. Keep English sources and their Japanese translations aligned, including commands, constraints, diagrams and examples. Update the relevant contract when behavior changes; use the [documentation index](docs/README.md) to find it.

## Reporting issues

Include the version, a minimal input, expected and actual behavior, and relevant reproduction commands. Use the [security policy](SECURITY.md) for suspected vulnerabilities rather than posting sensitive details publicly.
