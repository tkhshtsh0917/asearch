# asearch

Rust/WebAssemblyによるワイルドカード付き曖昧照合ライブラリです。[asearch.ts](https://github.com/teramotodaiki/asearch.ts) 1.0.3の互換移植を出発点に、短い検索入力と多数の候補文を扱うエンジンを開発します。

Rustコアとブラウザ／Node向けの薄いTypeScriptラッパを提供します。互換照合に加え、一括索引、容量制限付きの逐次検索セッション、サーバー生成のbase/deltaによるatomic更新を実装しています。意図表現もDAGで非展開のまま検索できます。残る受入項目は[開発計画](docs/PLAN.md)を参照してください。コンパイル済みpatternとWorkspaceの再利用を重視しています。Node・3ブラウザ・ネイティブの計測方法は[計測ハーネス](tools/bench/README.md)を参照してください。

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

`build`の入力は1行1文書のUTF-8 JSONLです。CLIのtextはUnicode scalar文字列を扱い、孤立サロゲートを含む入力にはRustのUTF-16 APIまたはWASMビルダを使います。例: `{"key":1,"texts":["利用方法を知りたい","使い方は？"]}`。任意の`order`はu32で、既定はkeyです。未知のフィールド・重複key・空のtextsを拒否し、入力を1行1 MiB・全体64 MiBに制限します。既定の日本語正規化・code pointでASIX baseを生成し、出力は新規作成だけを許します。`search`は同じ既定設定で検索し、上位30件と最小levelごとのtotalsをJSONへ出力します。`delta CHANGES_JSONL NEW_DELTA_ASIX`はupsert/removeの変更リストから不変segmentを作ります。`compact NEW_BASE_ASIX SEGMENT_ASIX...`は古い順の1〜8segmentへTOMBを適用し、live文書だけのbaseを生成します。`manifest INDEX_ID GENERATION NEW_MANIFEST_JSON PUBLIC_FILENAME=LOCAL_ASIX...`は各segmentを検証してbytes/texts/id/SHA-384を出力します。CLIは既定のcode-point/ja-basic-v1設定に対応し、公開URLにはASCII英数字・`.`・`-`・`_`からなる相対ファイル名を指定します。出力はすべて新規作成です。manifestの系列とgenerationの採番・公開順は呼出側で管理します。

M2のRust正規化基盤として`Normalizer::NONE`／`Normalizer::JA_BASIC`も利用できます。`normalize_into`は入力以上の容量を予約した`Vec<u16>`へ確保ゼロで書き込みます。これは単発Asearchの動作を変えず、一括索引とクエリに共通して適用します。

Rustでは`TransposedEngine`で複数textをまとめて照合できます。入力はUTF-16のsliceで、既定では日本語正規化を両側に施します。検索語は`QueryWorkspace`で準備し、`engine.workspace(&query, 7)`で照合用メモリを予約してから、`Engine::search`を呼びます。返値は入力text順の`Option<u16>`で、K以内に一致しないtextは`None`です。workspaceは予約したpattern長・K・ブロック容量以内で再利用でき、検索中の確保はありません。

文書単位では`AsearchIndex::from_docs`へ`Doc { key, order, texts }`を渡します。`texts`は同じ文書の言い換えなどのUTF-16 sliceで、1件以上が必要です。重複keyは拒否し、入力文書の並びに依存せず`(level, order, key)`順に検索結果を返します。`index.workspace(&query, 7)`で予約し、`index.search(&query, &mut workspace, 30, true)`の`hits`・`totals`を使えます。同じkeyは最良textだけを返し、totalsはlimitと無関係に最小levelごとの文書数を数えます。`text(hit.text_id)`と`original_text(hit.text_id)`で正規化後／前のtextを参照できます。

`index.to_bytes()`でASIX v1のbase segmentを保存し、`AsearchIndex::from_bytes(&bytes, options, limits)`で検証して再ロードできます。normalizerとprofileの記号単位はファイルと一致する設定を渡します。block_wordsは変更しても保存バイト列には影響しません。原文が無い外部索引では`original_text`は`None`です。XXH3は破損検査で、改ざんに対する真正性の保証ではありません。Rustの`from_delta(docs, tombstones, options, limits)`はTOMB付きsegmentを作れ、loaderは順序・重複・件数を検証して保持します。Rustの`IndexSet`で複数segmentを併合し、`append_bytes`／`replace_bytes`で検証後に一括採用できます。nativeでは更新前にworkspaceを解放します。JSでは自動で再準備します。

JS/TSの一括APIも同じRust実装を利用します。

```ts
import { AsearchIndex, fixed } from "asearch-wasm";

const index = AsearchIndex.fromDocs([
  { key: 1, texts: ["利用方法を知りたい", "使い方は？"] },
  { key: 2, texts: ["解約方法は？"] },
]);
try {
  const result = index.search("使い方", {
    maxAmbig: fixed(1),
    countTotal: true,
  });
  console.log(result.ids, result.levels, result.totals);
  const bytes = index.toBytes();
  // AsearchIndex.fromBytes(bytes)で同じbaseを再ロードできる。
  void bytes;
} finally {
  index.free();
}
```

`fromStrings`は添字をkeyにし、`buildIndex(docs)`はCLIと同じバイト列を返します。bulk APIの既定はIdeal/code pointです。`asearch.ts`／`asearch.ts-lib`も選べ、後者はtextをUTF-16単位で扱います。Rustで正規化・K・wrapを準備した後、JSがホストのlocale lower/upper候補を作り、Rustが検証して一括照合します。複数記号への大小文字展開は単発API同様に一致候補から除きます。一括結果のids/levels/refs/orders/totalsは現在JS所有のコピーで、`copy()`でさらに独立した複製を作れます。`explain(ref, pattern, options)`は原文UTF-16のspansとcoordinateSpaceを返し、原文が無ければnormalized-utf16を明示します。原文を上書き指定する場合は正規化後の内容が一致する必要があります。説明のDP行列が総予算に収まらない場合はRESOURCE_LIMITです。WASMビューは未実装です。非対応オプションを黙って無視せず拒否します。

`maxAmbig`には関数も渡せます。`({ normalized, mode, profile }) => number`へ内部wrapを含まない正規化済み文字列を一度だけ渡し、返値の整数0〜7を使います。非同期関数や範囲外の返値は拒否します。例えば`maxAmbig: ({ normalized }) => Math.min(2, Math.floor(Array.from(normalized).length / 5))`です。

逐次入力には独立した`SearchSession`を作れます。索引本体を共有し、query/workspaceと既定32MiB上限の行キャッシュをセッションごとに持ちます。キャッシュ容量はindex全体の予算へ合算し、収まらなければ全量検索へ戻ります。条件を満たす延長入力では完全なtext候補を再検証するNarrowingも選びます。stats.engineで実際の経路を確認できます。`maxCacheBytes: 0`または`engine: "transposed-blocked"`でキャッシュを無効化できます。親indexのfreeは子sessionも無効にします。

```js
const session = index.session({
  countTotal: true,
  maxCacheBytes: 8 * 1024 * 1024,
});
try {
  session.update("検");
  const result = session.update("検索");
  console.log(result.ids, result.stats.reusedRows);
  session.reset();
} finally {
  session.free();
}
```

`fromBytes/fromStream`の`expected`へmanifestのsegment descriptorを渡すと、bytes/texts/idとファイル全体のSHA-384をRustで検証してから返します。信頼できるmanifestのdescriptorを渡してください。XXH3だけでは改ざん対策にはなりません。

ネットワーク取得は呼出側が行い、取得したbodyを`AsearchIndex.fromStream`へ渡せます。ヘッダ64バイトを確認してからWASM内に固定バッファを予約し、chunkを直接コピーします。不足・超過・読み込み失敗ではreaderをcancel/releaseしcandidateを破棄します。`maxBytes`は非圧縮の宣言長に適用し、Content-Lengthには依存しません。Rustで輸送バッファを所有する場合は`from_owned_bytes`を使うと、その容量を構築中の予算にも含められます。

```ts
const response = await fetch(indexUrl);
if (!response.ok || !response.body) throw new Error("Index request failed");
const index = await AsearchIndex.fromStream(response.body, {
  maxBytes: 16 * 1024 * 1024,
});
try {
  console.log(index.search("検索語").ids);
} finally {
  index.free();
}
```

Rustでは`IndexSet::new(vec![base, delta], limits)`で不変segment列を検索できます。`set.workspace(&query, 7)`と`set.search(...)`を使い、`to_bytes()`で生きている文書だけのbaseへ圧縮します。原文あり/なしの生存文書を同じcolumnへ混ぜることはv1では拒否します。Rustの更新にはworkspaceをdropしてからappend_bytes/replace_bytesを呼び、同じqueryでworkspaceを再作成します。失敗した更新は旧column/revisionを保持します。JSの更新は下記addSegment/replaceSegmentsを使います。

サーバー側で差分を生成する場合は、`buildDelta(docs, tombstones)`でTOMB付きの不変segmentを作れます。更新対象keyをtombstonesへ含め、同じdeltaのdocsへ置換後のtextを渡します。削除だけならdocsを空にします。これは既存indexを編集するAPIではありません。

CLIでは`asearch-cli delta CHANGES_JSONL NEW_DELTA_ASIX`を使います。1行につき`{"op":"upsert","key":2,"texts":["新しい本文"]}`または`{"op":"remove","key":9}`を渡し、同じkeyの変更を重複させないでください。upsert/removeのkeyは自動でTOMBへ入り、nativeとWASMの出力は同じバイト列になります。manifestと圧縮のコマンドは上記CLIの説明を参照してください。

更新を使わない静的なWorker分割では、各Workerからids/levels/orders/refs/lengthだけをstructured cloneし、`mergeResults(parts, limit)`で順位順に統合できます。同じkeyは最良の1件だけを返し、返値のpartsがrefsを解決するWorkerを示します。copyメソッドをメッセージへ含めないでください。上限は256 shard、出力型配列は合計256MiBで、totalsは合算しません。TOMB更新を使う索引は1 Workerにまとめます。

配信manifestの比較には`diffManifest(current, next)`を使えます。系列・世代・descriptorを検証し、`stale`／`none`／`append`／`replace`を返す純関数です。取得やcurrentの保存は行いません。同一世代で内容が変わった場合や、既知のsegment IDの内容定義が変わった場合はCONFLICTで拒否します。完全な新列はreplaceSegmentsで一括採用し、成功後だけcurrentを保存します。複数addSegmentの連続呼出しをatomicなmanifest適用の代わりに使わないでください。

### 意図表現

```ts
import { IntentIndex } from "asearch-wasm";

const intents = IntentIndex.fromEntries([
  { key: 1, expr: "(ライト|電球)を(交換|修理)したい" },
]);
try {
  const result = intents.search("電球 修理", { engine: "dag", maxAmbig: 0 });
  if (result.length)
    console.log(intents.explain(result.refs[0], "電球 修理", { maxAmbig: 0 }));
} finally {
  intents.free();
}
```

リテラル、`()`、`|`、1回の後置`?`、次の文字のエスケープを扱います。`*`/`+`、繰り返しの`??`、閉じ忘れは拒否します。空の式/選択肢は空文字列です。分岐の全展開を生成せず、正規化やサロゲート対もグループ境界をまたいで処理します。`explain`は最良の原文展開を返します。`IntentIndex`は更新・保存・sessionを持たず、変更時は全量を再構築します。blockWordsは共通optionsの検証対象ですが、DAGのレイアウトには使いません。深さ・node/edge数・変換状態数と作業容量の上限は[D27](docs/DECISIONS.md)を参照してください。DAG経路の作業確保は、準備済みbulk検索の確保ゼロ保証と区別します。

### サーバー生成セグメントの更新

```ts
// 呼出側で信頼するmanifestを取得し、segmentをfetchする。
const response = await fetch(manifest.segments[1].url);
if (!response.ok || !response.body) throw new Error("Segment fetch failed");
await index.addSegment(response.body, manifest.segments[1]);
// 複数segmentを同時に入れ替える場合はreplaceSegmentsへ列全体を渡す。
// 例: await index.replaceSegments([{ source: baseBytes, expected: baseInfo }]);
```

`addSegment`／`replaceSegments`は受信・構造・予算・指定したexpectedのSHA-384検証がすべて成功してから採用します。受信中は旧索引を検索でき、二重更新はBUSY、freeは受信を中止します。失敗時は旧文書とrefsを維持し、成功時は全refsが失効します。子sessionは入力とオプションを保ち、次回updateで新しい文書集合を検索します。キャッシュは安全のため全体を再準備します。`segments()`でid/bytes/texts/dead、`toBytes()`でlive文書だけのcanonical baseを取得できます。CDN監視・取得・リトライ・manifest世代管理はライブラリ外です。

`buildDelta(docs, tombstones)`は既存索引を編集せず、サーバー用の不変segmentを生成します。追加・変更文書はdocsへ、変更・削除した既存keyはtombstonesへ渡します。変更文書は必ずtombstoneと新しい全textsを組にしてください。CLI deltaはupsertのkeyを自動でTOMBへ含めます。

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

規模確認では、AutoWikiQAの正規化済み質問1万件を反復した1,000万UTF-16単位・365,091文書で、既定の資源上限内で構築・ロード・検索を確認しました。Apple M5 Pro上の3クエリ（K=0/1、totalsあり、各20回warmup＋1,000回の個別検索）では、Node 24.22.0のp95が約1.6〜4.8ms、Chromium 156／Firefox 157／WebKit 27.2では約1.7〜6msで、結果・総件数も一致しました。Nodeの結果は単発Ideal検索の期待値とも照合しました。44,381,280バイトの索引はnative/WASMでも一致しました。これは反復コーパスと単一環境での観測で、ブラウザ全般の性能保証や実データ全体の検証ではありません。

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

ASIXローダのカバレッジ付きfuzzingは[専用workspace](fuzz/README.md)へ分離しています。通常のRust固定版とは別のnightlyを指定し、`mise run fuzz`でseed生成と30秒間のlibFuzzer/AddressSanitizer検証を実行できます。corpus・失敗artifactは追跡せず、CIでも同じ上限で実行します。

## ライセンスと由来

新規の追加・改変部分はMIT OR Apache-2.0です。移植元の表示と条件は[NOTICE](NOTICE)に保持しています。増井俊之氏のasearchアルゴリズム、asearch.ts、node-asearchの由来・同梱情報は[設計資料](docs/README.md)と[upstream](vendor/upstream/README.md)を参照してください。
