# ASIX fuzzing

ASIXの未信頼入力・構造検証・canonical roundtripをlibFuzzer＋AddressSanitizerで検査します。通常の製品ビルドと分離し、専用nightlyは`rust-toolchain.toml`、cargo-fuzzはmise taskで固定しています。[Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html)の構成に従います。

repoルートで実行します。

```sh
rustup toolchain install nightly-2026-10-20 --profile minimal
mise run fuzz
```

synthetic seedは`examples/seeds.rs`から生成し、corpus/artifactsはGit追跡しません。targetは入力を64 KiB、text数・単位数・working容量も小さく制限します。生入力に加え、長さとXXH3を再計算した入力も検査するため、checksum gateより深いセクション/原文/順序の検証へ到達できます。製品コードの検証は無効化しません。

短い実行で不具合が見つからなくても安全の証明にはなりません。発見した最小入力は原因を修正し、独立した回帰テストに加えます。CIでも上限を設けて実行し、速度の判定には使いません。

Sanitizer自身の解放済みメモリ保持は32 MiB、thread-localは64 KiBへ固定し、fuzzer全体のRSS上限は512 MiBとしています。これはtargetが指定する8 MiBのコア容量予算とは別です。Sanitizerの保持領域でRSS上限に達した場合も、使用中の確保と区別して原因を確認します。
