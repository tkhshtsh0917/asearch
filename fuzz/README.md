# Fuzzing

[日本語](README.ja.md)

The libFuzzer/AddressSanitizer targets check untrusted ASIX and intent inputs separately from product builds. `rust-toolchain.toml` in this directory pins nightly; the mise task pins cargo-fuzz. See the [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html).

Run from the repository root:

```sh
rustup toolchain install nightly-2026-10-20 --profile minimal
mise run fuzz
```

`asix` checks structural validation and canonical round trips with inputs up to 64 KiB and small text/unit/working limits. Seeds come from `examples/seeds.rs`. It checks both raw input and input with recomputed length/XXH3, reaching deeper sections, originals and ordering without disabling product validation. Its core capacity budget is 8 MiB.

`intent` checks parsing, normalization/surrogate conversion, DAG matching and the best expansion's explanation with inputs up to 512 bytes. Its limits are 8 entries, 1,024 units, 4,096 slots and 2 MiB working memory. Seeds come from `examples/intent_seeds.rs`.

The sanitizer quarantine is 32 MiB with a 64 KiB thread-local quarantine; the fuzzer RSS limit is 512 MiB. These are separate from core capacity budgets. Diagnose sanitizer retention separately from live allocations. Corpus, artifacts and coverage outputs are ignored by Git.

Finite fuzz runs do not prove safety. Minimize findings and add independent regressions after fixing them. CI bounds execution time and does not use fuzz speed as a performance gate. See [Contributing](../CONTRIBUTING.md).
