# Asearch documentation

[日本語](ja/README.md)

| Document                              | What it explains                                                                                                   |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| [Architecture](architecture.md)       | How the five Rust crates cooperate with a service's server, CDN and browser; ownership of updates and deployment.  |
| [Bitap walkthrough](algorithm.md)     | An animated, numerical example of exact and approximate matching, followed by its relationship to the bulk engine. |
| [ASIX v1 byte layout](asix-format.md) | Every header offset, section-table entry, payload type and a reproducible 256-byte example.                        |

The [root README](../README.md) covers installation, CLI commands and releases. Run `cargo doc --workspace --no-deps` for Rust API details. JS/TS bindings, browser integration and their contracts live in [asearch-demo/integration](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration).

The SVG diagrams remain sharp when enlarged. GIF walkthroughs also have static PNG frames and text equivalents. Both languages use the same byte layout and numerical examples.
