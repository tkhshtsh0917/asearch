# fuzz検証

[English](README.md)

libFuzzer／AddressSanitizerのtargetは、製品ビルドと分離して未信頼のASIX・意図式の入力を検証します。このディレクトリの`rust-toolchain.toml`でnightly、mise taskでcargo-fuzzを固定します。[Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html)も参照してください。

リポジトリのルートで実行します。

```sh
rustup toolchain install nightly-2026-10-20 --profile minimal
mise run fuzz
```

`asix`は最大64 KiBの入力と小さなtext数・単位数・working上限で、構造検証とcanonical roundtripを調べます。seedは`examples/seeds.rs`から生成します。生入力と長さ・XXH3を再計算した入力の両方を使い、製品の検証を無効化せずに深いセクション・原文・順序へ到達します。コアの容量予算は8 MiBです。

`intent`は最大512バイトの入力で、parser・正規化／サロゲート変換・DAG照合・最良展開の説明を検証します。上限は8 entry・1,024 units・4,096 slots・2 MiB workingです。seedは`examples/intent_seeds.rs`から生成します。

Sanitizerのquarantineは32 MiB、thread-localのquarantineは64 KiB、fuzzerのRSS上限は512 MiBです。コア容量の予算とは別で、Sanitizerの保持と使用中の確保を区別して原因を調べます。corpus・artifacts・coverageの出力はGitで追跡しません。

有限回のfuzzで安全は証明できません。発見した入力を最小化し、修正後は独立した回帰検証へ加えます。CIは実行時間を制限し、fuzzの速度を性能のゲートには使いません。[コントリビューション](../CONTRIBUTING.ja.md)も参照してください。
