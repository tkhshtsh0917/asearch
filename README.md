# asearch

Rust/WebAssemblyによるワイルドカード付き曖昧照合ライブラリです。[asearch.ts](https://github.com/teramotodaiki/asearch.ts) 1.0.3の互換移植を出発点に、短い検索入力と多数の候補文を扱うエンジンを開発します。

M0の互換移植とM1の拡張を実装しました。Rustコアとブラウザ／Node向けの薄いTypeScriptラッパを提供します。一括索引、逐次入力、サーバー生成セグメントの更新は後続の[開発計画](docs/PLAN.md)にあり、まだ利用できません。コンパイル済みpatternとWorkspaceの再利用を重視しています。Node・3ブラウザ・ネイティブの計測方法は[計測ハーネス](tools/bench/README.md)を参照してください。

構成と依存方向は[リポジトリ構成](docs/repository-layout.md)を参照してください。Rust本体・JS配布層・テスト・将来の利用例を分離しています。

## ビルド

Git、mise、rustupが必要です。Rustの版は`rust-toolchain.toml`、Node／pnpmの版は`mise.toml`で固定しています。リポジトリのルートで実行してください。

```sh
mise trust
mise install
rustup show active-toolchain
cargo install wasm-bindgen-cli --version 0.2.129 --locked
mise exec -- pnpm install --frozen-lockfile
mise exec -- pnpm build
```

`rustup show`は指定されたRust、rustfmt、Clippy、WASMターゲットを導入します。グローバルの既定Rustを変更する必要はありません。`wasm-bindgen-cli`は`Cargo.lock`の`wasm-bindgen`と同じ版にしてください。

出力は`packages/asearch-wasm/dist/`です。WASMは基本命令版のみで、SIMDは要求しません。ビルドスクリプトは埋め込まれるコンパイル時の絶対パスを一般化します。npmへの公開はビルドに不要です。

## 使い方

Node ESMのローカル成果物を使う例です。CommonJSでは`dist/node.cjs`を`require`します。Nodeは同期初期化します。

```js
import { Asearch, MatchMode } from "./packages/asearch-wasm/dist/node.mjs";

const pattern = new Asearch("猫である", MatchMode.Include);
console.log(pattern.match("吾輩は猫である。")); // true
console.log(new Asearch("hello").match("helo", 1)); // true
```

ブラウザでは`dist/`全体をHTTP(S)で配信し、`web.mjs`を読み込みます。相対配置の`web/asearch_wasm.js`と`web/asearch_wasm_bg.wasm`も必要です。`.wasm`は`application/wasm`で返してください。

```html
<script type="module">
  import { Asearch, ready } from "/assets/asearch/web.mjs";

  await ready;
  console.log(new Asearch("hello").match("helo", 1));
</script>
```

トップレベルawaitによって自動初期化します。失敗してもモジュールのimportは成功し、`ready`がrejectします。`init(new URL("/assets/asearch/web/asearch_wasm_bg.wasm", location.href))`で再試行できます。`ready`は現在の試行を指すlive bindingで、保存済みの古いPromiseは変わりません。同時の引数なし`init()`は同じPromiseを返します。初期化中や成功後に明示入力で再初期化することはできません。詳細は[パッケージREADME](packages/asearch-wasm/README.md)を参照してください。

DOMやNodeの`process`に依存せず、モジュールWorkerでも使えます。CSPでモジュールの読み込み、WASMの取得・コンパイルを許可する必要があります。UI、IME制御、入力の間引き、通信処理は呼び出し側の責務です。

Rustでは`crates/asearch-rs`をpath依存に指定して使えます。

```rust
use asearch_rs::{Asearch, MatchMode};

fn main() -> Result<(), asearch_rs::PatternTooLong> {
    let pattern = Asearch::new("hello", MatchMode::Exact)?;
    assert!(pattern.is_match("helo", 1));
    Ok(())
}
```

ネイティブCLIは検証・計測に使えます。

```sh
cargo run --locked -p asearch-cli -- verify-fixtures crates/asearch-rs/tests/fixtures/compat.json
mise exec -- pnpm bench:native OUTPUT_NATIVE_JSON 10000
cargo install --locked --path crates/asearch-cli
asearch-cli build docs.jsonl questions.asix
asearch-cli search questions.asix '検索語'
```

`verify-fixtures`は生成済みfixtureのコンパイル済みmaskから、両互換profileの全照合結果を検査します。constructor例外のケースは件数を表示してスキップし、constructorそのものはRust/Nodeテストで検査します。`bench:native`はCLIをreleaseビルドしてからネイティブの最小level走査を計測し、raw標本と集計を保存します。JS/WASM境界・構築・key順位付けを含まない測定です。詳細は[計測ハーネス](tools/bench/README.md)を参照してください。

`build`の入力は1行1文書のJSONLです。例: `{"key":1,"texts":["利用方法を知りたい","使い方は？"]}`。任意の`order`はu32で、既定はkeyです。未知のフィールド・重複key・空のtextsを拒否し、入力を1行1 MiB・全体64 MiBに制限します。既定の日本語正規化・code pointでASIX baseを生成し、出力は新規作成だけを許します。`search`は同じ既定設定で検索し、上位30件と最小levelごとのtotalsをJSONへ出力します。delta/compactコマンドは後続段階で実装します。

M2のRust正規化基盤として`Normalizer::NONE`／`Normalizer::JA_BASIC`も利用できます。`normalize_into`は入力以上の容量を予約した`Vec<u16>`へ確保ゼロで書き込みます。これは単発Asearchの動作を変えず、一括索引への接続は実装中です。

Rustでは`TransposedEngine`で複数textをまとめて照合できます。入力はUTF-16のsliceで、既定では日本語正規化を両側に施します。検索語は`QueryWorkspace`で準備し、`engine.workspace(&query, 7)`で照合用メモリを予約してから、`Engine::search`を呼びます。返値は入力text順の`Option<u16>`で、K以内に一致しないtextは`None`です。workspaceは予約したpattern長・K・ブロック容量以内で再利用でき、検索中の確保はありません。

文書単位では`AsearchIndex::from_docs`へ`Doc { key, order, texts }`を渡します。`texts`は同じ文書の言い換えなどのUTF-16 sliceで、1件以上が必要です。重複keyは拒否し、入力文書の並びに依存せず`(level, order, key)`順に検索結果を返します。`index.workspace(&query, 7)`で予約し、`index.search(&query, &mut workspace, 30, true)`の`hits`・`totals`を使えます。同じkeyは最良textだけを返し、totalsはlimitと無関係に最小levelごとの文書数を数えます。`text(hit.text_id)`と`original_text(hit.text_id)`で正規化後／前のtextを参照できます。

`index.to_bytes()`でASIX v1のbase segmentを保存し、`AsearchIndex::from_bytes(&bytes, options, limits)`で検証して再ロードできます。normalizerとprofileの記号単位はファイルと一致する設定を渡します。block_wordsは変更しても保存バイト列には影響しません。原文が無い外部索引では`original_text`は`None`です。XXH3は破損検査で、改ざんに対する真正性の保証ではありません。現在のloaderはbase専用で、TOMBを持つdelta・JS一括API・streamは実装中です。

## 互換性と制限

Rustの拡張経路は`ScalarWm<W>`です。`W`には`u32`／`u64`／`u128`／`MultiWord<N>`を指定でき、非ワイルドカード記号数＋開始位置が語の容量に収まる必要があります。例えば`MultiWord<2>`は127記号まで扱えます。

```rust
use asearch_rs::{MatchMode, MultiWord, ScalarLimits, ScalarWm};

fn main() -> Result<(), asearch_rs::ScalarError> {
    let query = "検索".repeat(40);
    let matcher = ScalarWm::<MultiWord<2>>::new(
        &query, MatchMode::Exact, ScalarLimits::default(),
    )?;
    let mut workspace = matcher.workspace(3)?;
    assert_eq!(matcher.match_level(&query, &mut workspace)?, Some(0));
    Ok(())
}
```

この経路はcode point単位のIdeal意味論で、正規化せず、Rustの単一記号になる大小文字候補を使います。既定の入力上限は65,535 UTF-16単位、作業容量は512 MiBです。構築時には一時配列を含めて検査し、照合時にはmatcher＋渡されたworkspaceの予算を検査します。予約後の照合は確保ゼロです。複数workspaceや別instanceの合計、呼出側の入力、プロセス全体のメモリはこの単体APIの予算に含みません。JS/WASMの互換APIの31単位制限とは別のAPIです。

- `Exact`、`Include`、`WildcardSpace`を実装し、patternの上限は空白を含め31 UTF-16単位です。
- 既定の`asearch.ts` profileはtextをcode pointで走査します。`{profile: "asearch.ts-lib"}`は公開された上流JSのUTF-16挙動を再現します。`{profile: "ideal"}`はcode pointで走査し、正しい初期閉包による単調なLevenshtein照合を行います。どのprofileもこのAPIのpattern上限は31 UTF-16単位です。
- `match(text, ambig)`はそのlevelの真偽です。互換の2profileではlevelを増やしても結果が単調になるとは限らず、32以上のシフト折り返しも再現します。idealは単調で、32以上でも折り返しません。`matchLevel(text, maxAmbig=3)`は最初の一致level、無ければ-1を返し、maxAmbigには整数0〜31を指定します。
- pattern／textは文字列またはStringオブジェクトを受理し、それ以外はTypeErrorです。levelは65,535までで、それ以上はRangeErrorです。上流の偶発的な`ambig='length'`の挙動は再現しません。
- 公開プロパティを書き換えてもコンパイル済みの照合状態は変わりません。JSの大文字小文字変換はホストのlocaleに従います。Rust単体はRustのUnicode変換を使い、ホストlocaleの完全再現にはJS生成のmaskを`from_parts`へ渡します。
- 通常のRust照合（level 0〜31）は確保ゼロを検査します。WASM境界の文字列コピー／一時確保、GCによる解放の遅延はあります。M1で`free()`と`[Symbol.dispose]()`を追加しました。解放後の照合は`DISPOSED`エラーです。TypeScriptのlib設定には`ESNext.Disposable`を含めてください。
- 自動検証対象はNode 22／24とPlaywrightのChromium／Firefox／WebKitです。スマートフォン実機、IME／描画の挙動は未検証です。

## 開発・検証

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
mise exec -- pnpm fixtures:check
mise exec -- pnpm typecheck
mise exec -- pnpm lint
mise exec -- pnpm docs:check
mise exec -- pnpm test
mise exec -- pnpm exec playwright install chromium firefox webkit
mise exec -- pnpm test:browser
```

ビルド後にJS／型／ブラウザテストを実行します。LinuxでブラウザのOS依存ライブラリも必要な場合は`playwright install --with-deps chromium`を使います。fixture生成・検査は`LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8`で実行します。

JS／TS／設定はBiome、Markdown／YAMLはPrettierと文書リンク検査、Rustはrustfmt／Clippyを使います。自動修正は`pnpm fmt`、`pnpm docs:fmt`、`cargo fmt --all`です。生成物・上流コピーは整形対象から除外しています。VS Codeの推奨拡張と保存時整形、Git追跡状態の表示設定も同梱しています。

上流コピーのハッシュを検証し、固定seedの3,168ケースを両profileでRustとNodeから照合します。上流の36 assertion、型互換、Rustの確保回数、初期化失敗／再試行、Worker、tarballを別ディレクトリへ展開した配布検査も含みます。fixtureを更新する場合は`pnpm fixtures`を実行して差分をレビューしてください。期待値生成だけは上流変換を再現するTypeScript 5.9.3を専用workspaceに隔離し、製品ラッパの型検査にはTypeScript 7を使います。

## ライセンスと由来

新規の追加・改変部分はMIT OR Apache-2.0です。移植元の表示と条件は[NOTICE](NOTICE)に保持しています。増井俊之氏のasearchアルゴリズム、asearch.ts、node-asearchの由来・同梱情報は[設計資料](docs/README.md)と[upstream](vendor/upstream/README.md)を参照してください。
