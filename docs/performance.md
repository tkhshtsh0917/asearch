# Performance decisions

[日本語](ja/performance.md)

The pre-release tuning pass kept ASIX v1, exact totals, ranking, original-coordinate explanations, resource limits and atomic update semantics unchanged. Three changes were retained:

| Change                                                             | Observed benefit                                                                    | Scope                                                                                                                                                                                              |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Scalar Bitap for complete sparse candidates                        | Roughly 40–60% less time in the targeted warmed repeated-update comparisons         | A short query over a 1,001-document, roughly 2-million-unit synthetic corpus with one retained candidate. Browser samples are batch means of public updates, not individual keystroke percentiles. |
| Reuse already-normalized owned rows                                | Roughly 17–22% less in-memory load time                                             | The demo's fixed 10,000 AutoWikiQA questions, including expected SHA-384 verification; excludes HTTP and WASM initialization.                                                                      |
| Find explanation membership through existing document ranges/masks | Roughly 60–70% less time for a group of 20 explanations in the late-hit stress case | A 10,000-document synthetic corpus whose matching documents occur at the end. The ordinary Japanese query showed only a small difference.                                                          |

These are local Node/Chromium/Firefox/WebKit observations on one macOS arm64 development machine, using paired AB/BA runs and separate confirmation measurements. They are **conditional improvements**, not a general speedup factor for a website. The ordinary 10,000-question typing workload showed much smaller search differences. A browser batch average, an individual API call, a network-inclusive startup and a DOM update measure different things. Detailed raw runs and diagnostic experiments are not part of the product contract.

## Reproducing comparisons

The Rust repository owns the raw CLI/WASM and Rust correctness tests. The [integration performance guide](https://github.com/tkhshtsh0917/asearch-demo/blob/main/integration/docs/performance.md) owns explicit before/after distributions, seeded traces, load/explanation measurements and browser comparisons. The [demo measurement guide](https://github.com/tkhshtsh0917/asearch-demo/blob/main/docs/performance.md) covers Worker round trips and accepted DOM mutations.

Build both revisions with the same locked toolchain and dependencies. Keep source commit/diff identity, raw asset and generated artifact hashes, corpus/workload hashes and flags. Prepare fixtures before timing, compare full outputs outside timing, preserve zero clock samples and resample whole paired runs. Confirm in another process and inspect both medians and tails. Do not compare unrelated historical JSON files as if they were paired measurements.

Functional coverage includes ASIX round trips, Unicode/casing/normalization, sparse-query word boundaries, old-reference rejection, update failures and emulated desktop/mobile browser profiles. Finite tests and fuzz runs do not prove all inputs safe, and emulation does not measure phone hardware performance. Network/CDN behavior and service-specific workloads still need deployment-level evaluation.
