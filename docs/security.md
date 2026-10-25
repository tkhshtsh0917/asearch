# Security and dependency maintenance

[日本語](ja/security.md)

This repository ships Rust source, native CLI binaries and a read-only search WASM. Its Cargo lockfiles contain no Brotli dependency. Hosts own their HTTP stacks, browser/runtime updates and deployment settings; Rust dependency audits do not cover those components.

```sh
mise run audit
```

The pinned cargo-audit tool checks both Cargo.lock and fuzz/Cargo.lock against the current RustSec database. CI includes these checks before a Release can be published, and a weekly/manual workflow checks for newly reported issues. Database/network failures fail the check. Dependabot proposes Cargo and GitHub Actions updates; review and test them before merging. An audit with no findings only describes known advisories at that time.

The companion demo manages Node and TS dependencies separately. See its [security guide](https://github.com/tkhshtsh0917/asearch-demo/blob/main/docs/security.md), including the patched Node requirement for CVE-2026-111497.

Report suspected vulnerabilities through GitHub private vulnerability reporting when enabled. If unavailable, request a private contact channel without disclosing vulnerability details. Do not put credentials or sensitive reproductions in public issues.
