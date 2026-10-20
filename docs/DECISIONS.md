# DECISIONS — 判断記録

仕様からの逸脱、未決事項、設計判断を記録する。D1〜D8 は 2026-10-19 に利用者確認・決定済み、D9は2026-10-20にサーバー配信のみで確認済み（D18）、D10 は M0 の既定として提案（異論があれば変更する）。D11〜D17 は2026-10-20の仕様整備に伴う具体化（未実装）。D18が旧overlay案より優先する。1 件ごとに以下の形式で追記する（古い項目は消さず、状態を更新する）。

```text
## Dn: <題名>
- 状態: 提案 | 利用者確認済み | 却下 | 置き換え（→ Dm）
- 日付: YYYY-MM-DD（JST）
- 背景:
- 選択肢: A / B / ...
- 決定（または推奨）と理由:
- 検証方法（テスト名、ベンチ、参照スクリプト）:
- 影響範囲:
```

---

## D1: compat `Asearch` の既定プロファイル

- 状態: 利用者確認済み（A を採用）
- 日付: 2026-10-19（確認 2026-10-19 00:27 JST）
- 背景: npm 公開物 asearch.ts@1.0.3 は ES5 降格で text を UTF-16 単位で走査するため、非 BMP 文字（絵文字、𠮷 など）が曖昧度 0 で一致しない。TypeScript ソースと README は code point 単位を意図している（SPEC §2.4）。
- 選択肢: A) `AsearchTs`（code point） B) `AsearchTsLib`（公開物とビット一致）
- 決定: A。違いは非 BMP 文字を含む場合だけで、B の挙動は明らかな不具合。B は `options.profile: 'asearch.ts-lib'` で選べるようにする。
- 検証方法: フィクスチャで、A は非 BMP を含むケース以外で公開物と一致、B は全ケースで一致。

## D2: 一括 API の既定の曖昧度ポリシー

- 状態: 利用者確認済み（A を採用）
- 日付: 2026-10-19（確認 2026-10-19 00:28 JST）
- 背景: 短い pattern で K を大きくすると、ほぼ全件が一致して順位付けの意味が薄れる。takker の Scrapbox 用エンジンは `min(floor(len/4)+1, 4)` 段（最大 level は `min(floor(len/4), 3)`）。
- 選択肢: A) `ByLength{every:4,max:3}` B) `Fixed(k)`
- 決定: A。K = min(floor(len/4), 3)（3 文字以下は完全一致のみ、4〜7 文字で 1、8〜11 文字で 2、12 文字以上で 3）。呼び出し側で上書きできる。
- 影響範囲: K が増える打鍵（4・8・12 文字目）では Narrowing を使えず全量計算になる（SPEC §6.1）。Incremental は K_cache を最大 K で持てば再計算を避けられる（SPEC §4.7）。

## D3: パッケージ・クレート名

- 状態: 利用者確認済み
- 日付: 2026-10-19（確認 2026-10-19 00:40 JST）
- 背景: crates.io の `asearch` は takker99/rust-asearch が使用中。Rust 本体が主役で、WASM は配布先の一つという位置づけ。
- 決定: リポジトリ名 `asearch`、本体 crate `asearch-rs`（lib 名 `asearch_rs`）、npm パッケージ `asearch-wasm`。ワークスペース内の補助 crate は `asearch-wasm`（wasm-bindgen 境界、npm 生成用）と `asearch-cli`。
- 空き確認: 2026-10-19 00:33〜00:37 JST に `npm view` と `cargo search` で、npm `asearch-wasm` 未登録、crates `asearch-rs`・`asearch-wasm`・`asearch-cli` 完全一致なし。
- 注意: Rust API Guidelines（C-CASE）は crate 名に `-rs` 接尾辞を付けないことを推奨しているが、強制ではない。`asearch` が使用済みのため区別を優先した。

## D4: 一括 API の正規化の既定

- 状態: 利用者確認済み
- 日付: 2026-10-19（確認 2026-10-19 00:42 JST）
- 背景: 一括 API（AsearchIndex / SearchSession / IntentIndex）は asearch.ts に無い新機能なので、互換性より日本語での実用を優先する。特に IME で入力される全角空白がワイルドカードにならないと、複数語のクエリが 0 件になる。当初の推奨（`Identity`）から変更した。
- 決定: 一括 API の既定は `ja-basic-v1` の項目 1〜5（全角空白→半角、全角 ASCII→半角、半角カナ→全角、カタカナ→ひらがな、ASCII 大文字→小文字）を有効、項目 6（長音・ダッシュ類の統一）は無効。互換クラス `Asearch` は常に正規化なし（asearch.ts と同じ結果）。
- 影響範囲: 一括 API の結果は、asearch.ts を全件に適用した結果とは一致しなくなる（`normalizer: 'none'` と同じprofile/mode/wrapで、0..Kの最小levelを比較すれば一致する。match(text,K)だけでは非単調性により異なる。今回説明を訂正）。索引ヘッダに normalizer の ID・版・設定を記録し、不一致ならロードを拒否する（SPEC §8）。

## D5: 一括 API の既定モード

- 状態: 利用者確認済み（A を採用）
- 日付: 2026-10-19（確認 2026-10-19 00:44 JST）
- 背景: re_expand のデモや takker のエンジンは、入力を `q` のように空白で包み、空白をワイルドカードとして部分一致させている。
- 選択肢: A) `WildcardSpace` ＋ `wrap: true` B) asearch.ts と同じ `Exact`
- 決定: A。D4 の正規化で全角空白も半角空白として扱われ、ワイルドカードになる。受理位置に常に自己ループがあるので末尾ループの前提は満たすが、Narrowingには延長・K非増加等の条件も必要（SPEC §6.1。今回説明を訂正、D5のmodeは変更しない）。asearch.ts の Exact/Include が必要な場合は `mode` で指定する。

## D6: ロードマップ

- 状態: 利用者決定
- 日付: 2026-10-19（2026-10-19 00:48 JST）
- 決定: まず asearch.ts@1.0.3 を Rust ＋ WASM へ忠実に移植して initial commit とする（PLANのM0、SPEC §19）。本書で議論した改善（転置 DP、Incremental、Narrowing、索引、正規化、DAG など）は M1 以降で 1 つずつ入れる。
- 理由: 互換の基準点をコミット履歴に固定し、以後の改善をすべて「M0 と同じフィクスチャが通るか」で検証できるようにする。
- 影響範囲: 旧 M0（基盤）と旧 M1（互換コア）を新 M0 に統合し、`matchLevel`・明示 `free`・語幅ジェネリック化・ベンチ基盤は新 M1 に移した。正規化は M2。

## D7: プロジェクトのライセンス

- 状態: 利用者確認済み（A を採用）
- 日付: 2026-10-19（確認 2026-10-19 00:55 JST）
- 背景: 移植元は asearch.ts（ISC 宣言）、node-asearch（BSD-2-Clause 宣言）、asearch-ruby（MIT）。いずれも寛容なライセンスで、帰属表示（NOTICE）を置けば再ライセンスに制約は少ない。
- 選択肢: A) MIT OR Apache-2.0（Rust エコシステムの慣行） B) ISC（asearch.ts と同じ） C) MIT
- 決定: A（`MIT OR Apache-2.0`）。リポジトリに `LICENSE-MIT` と `LICENSE-APACHE` を置き、Cargo.toml は `license = "MIT OR Apache-2.0"`、package.json は `"license": "(MIT OR Apache-2.0)"`。移植元の表示は `NOTICE` に置く。

- 2026-10-20追記（利用者指定）: 本プロジェクトの追加・改変部分の著作権表記は `Copyright (c) 2026 Toshinori Takahashi`。移植元のコードや同梱コピーの権利をこの表記で置き換えない。既存のライセンス・帰属表示を保持し、出典はNOTICEへ記載する。ISCの表示条件は [OSIのライセンス本文](https://opensource.org/license/isc) を確認した。上流に無い著作権行を推測で作らない。

## D8: リポジトリのスコープ

- 状態: 利用者決定（大枠）。範囲内外の細目と想定規模は提案
- 日付: 2026-10-19（指示 2026-10-19 03:30 JST）
- 背景: 議論が DOM 描画、ローマ字、形態素解析、配信基盤にまで広がり、終わりが見えなくなるため、スコープを定義する。
- 決定（利用者）: 本リポジトリは「WASM による世界最速の曖昧照合」に限る。ただし、サーバー側で WASM が扱いやすい形式に前処理したバイナリを CDN で配信する使い方と、更新頻度が高い場合の対応はサポートする。
- 細目（提案）: SPEC §0。判断基準は「(a) 照合結果か、(b) 照合までの時間（ロードと更新を含む）に直接効くものだけを入れる。それ以外は拡張点だけ用意する」。
- 範囲外とした主なもの: DOM 更新とハイライト描画、入力・IME・防振・latest-wins のスケジューリング、言語資源（形態素解析、読み付与、ローマ字表、同義語辞書、意図表現の自動生成）、配信基盤（CDN 設定、マニフェストの取得・ポーリング・プッシュ、オフライン保存）、サーバー側検索、ランキング学習、長文全文検索。README の手順や非規範の例は可。
- 想定規模（提案）: 総 code unit 数 10^5〜10^6 が性能目標の主対象、10^7 が上限。1 text は 65,535 code unit 以下。
- 影響範囲: AGENTSとPLAN、SPEC §0・§9・§11・§12.3・§14〜§17・§20。これまでの議論のうち範囲外になったもの（DOM 操作の高速化、コーパス側のローマ字展開、読みの自動付与）は実装しない。

## D9: 索引の配信形式と更新方式

2026-10-20確認済み（D18で範囲を確定）。正規化済み原文・長さ・key・order・tombstoneを保存し、記号表・EQ・ブロック配置をロード時に構築する。派生構造の再配信を避け、更新の変化を文書単位へ閉じ込める。

サーバー生成のbase/delta/tombstoneとJSON manifestを読む。クライアント編集層は持たない。独自のバイナリパッチは作らず、配信圧縮は補助とする。製品でのロード時間、更新の反映時間、配信サイズはM2/M4で検証する。

## D10: M0 の互換境界の細目（asearch.ts からの逸脱を含む）

- 状態: 提案（M0 の既定。Codex の作業はこれに従い、利用者が変更を求めたら改める）
- 日付: 2026-10-20（JST）
- 背景: M0 の事前点検（Codex の立場で読み通す模擬）で、境界の挙動に実装者ごとに分かれる点が見つかった。同梱の上流 `vendor/upstream/` で実測し（2026-10-20 01:05〜01:11 JST、Node v22.22.0）、次のとおり決めた。詳細は SPEC §19.3〜§19.7。
- 方式: コンストラクタは asearch.ts の JS 本体をそのまま再現し、WASM には出来上がった状態（1 code point のキーの masks、loopMask、acceptPattern、initPattern）だけを渡す。mode の解釈、大文字小文字、`MAXLENGTH` の書き換えまで構成上一致する。Rust の遷移は u32 で、シフト量は 32 で割った余り（`wrapping_shr`）。
- ambig: asearch.ts の `Array.from({ length: ambig + 1 })` と `states[ambig]` を配列なしで再現する（"1" と [1] は 1、"01"・1.5・-1・null・true は false、長さが 2^32−1 を超えたら RangeError）。
- 逸脱（すべてテストで「仕様どおりの逸脱」として検査する）:
  1. pattern と query は文字列か String オブジェクトだけ。それ以外は TypeError（asearch.ts は数値を空文字列、配列を要素の列として扱う）。
  2. ambig の添字が 65,535 を超えたら RangeError（asearch.ts は計算を続ける。4294967294 では Node のプロセスが致命的なメモリ不足で落ちた。観測）。
  3. 構築後にインスタンスのプロパティを書き換えても照合に反映しない（asearch.ts は毎回読み直す）。
  4. `process.env` を参照せず、verbose を実装しない（Q6）。上流テストの `ASEARCH_VERBOSE` の前処理は移植しない。
  5. フィクスチャは en-US ロケールで生成する（`İ` の小文字化がロケールで変わる。tr-TR では結果が変わることを観測）。
  6. 既定プロファイル `AsearchTs` は text を code point で走査する（D1 の再掲）。
  7. ambig の文字列表現が `'length'` のとき（`'length'`、`['length']`、`toString` が `'length'` を返すオブジェクト）、asearch.ts は内部配列の長さを照合結果に使ってしまう（実装の偶然。31 文字の pattern では true になりうる）。本実装は false を返す（2 回目の事前点検で発見。8,820 通りの差分比較のうち、これ以外の食い違いは 65,535 超だけだった）。
- 検証方法: SPEC §19.6 のフィクスチャ（lib／src の 2 系統の期待値）と §19.7 の差分テスト。

## D11: 初期化試行とreadyの契約

- 状態: 設計具体化（今回の仕様整備依頼に基づくM0受入条件。未実装）
- 日付: 2026-10-20（JST）
- 背景: 失敗したreadyが残ったままinit成功を知らせる契約が曖昧だった。
- 決定: SPEC §19.5.1。readyは最後の試行のlive binding。同時initは引数なしなら同じPromise、明示入力の競合は拒否。成功後は再インスタンス化しない。Nodeは同期初期化のまま。
- 検証: I0。失敗→成功、保持済みPromise、競合、未処理rejection、pack経由のweb/Node。
- 影響: M0のみの初期化境界。asearchの照合意味論は変更しない。

## D12: 資源予算・更新失敗（overlay部分はD18で廃止）

- 状態: 設計具体化。D9確認済み。旧overlay関連の記述はD18で廃止。既定予算は暫定・未計測
- 日付: 2026-10-20（JST）
- 背景: 入力ファイル長だけでは派生メモリを制限できず、live textが多いoverlayはcompactionだけで小さくならない。
- 決定: SPEC §11.6/§12.4/§20.4。入力・個数・スロット・全session/候補の容量を検査し、超過は旧状態を保持してRESOURCE_LIMIT。mutableのcompact契機と容量上限を区別する。
- 検証: R2/U4、small-budget fault injection、native/WASMピーク計測。実ホストのOOMを意図的に起こさない。
- 影響: M2〜M4の構築・ロード・更新・結果所有権。無制限の入力/メモリ保証はしない。

## D13: ByLengthの長さの定義

- 状態: 設計具体化（D2/D4/D5の既定値は維持）
- 日付: 2026-10-20（JST）
- 決定: SPEC §10。正規化後・wrap前、既定はprofile単位、WildcardSpaceのユーザー空白も除外。Kは元のfloor(len/4), max=3。unit/includeWildcardsは上書き可。
- 検証: Q2/S3。3/4・7/8・11/12、全角・半角濁点・非BMP、mode/空白、正規化後LCP。
- 影響: M2の既定policyとM3のセッション。互換Asearchは正規化なし。

## D14: manifestの系列と適用完了状態

- 状態: 設計具体化（D9はD18の範囲で確認済み）
- 日付: 2026-10-20（JST）
- 背景: SegmentInfoと世代だけでは系列/設定/descriptorの不整合を検査できない。
- 決定: SPEC §20.5。indexId、系列内の不変index設定、safe integer generation、同世代の不変性、共有IDのdescriptor一致。diffManifestは完全な適用済みmanifest同士を比較する。取得は範囲外、expectedを渡したロードではSRIも検証する。複数segmentは一括採用後だけcurrentを更新する。
- 検証: M4、rollback/同世代改変/別系列/設定変更/内容不一致/途中失敗→再試行。
- 影響: M4の公開型・CLI manifest。暗号学的真正性は信頼したmanifest/通信経路が前提。

## D15: 文書単位の総件数

- 状態: 設計具体化（既存key単位ランキングと整合させる）
- 日付: 2026-10-20（JST）
- 決定: SPEC §4.4。totals[e]は最小level=eの生存key数。累積levelや複数の言い換えを重複加算しない。countTotal=falseは全件計数を省く。
- 検証: Q2/U4。1key複数text、異なるlevel、削除、複数segment、limit=0でも総件数不変。
- 影響: M2/M4の抽出・merge・計測。

## D16: 性能回帰の再現可能な判定

- 状態: 設計具体化（10%を回帰候補の閾値として維持）
- 日付: 2026-10-20（JST）
- 背景: 共有実行環境ではノイズがあり、単回の差を性能回帰と判定しない。
- 決定: SPEC §15.1.1。意味論の等しいpaired AB/BA計測、run単位の区間推定、追加独立測定で確認。共有CIは速度閾値を必須にしない。個別操作とバッチ平均の分布を分ける。
- 検証: P1、保存したrawから同じ集計を再現。ノイズ・明確な悪化を合成データで検査。
- 影響: M1以降のベンチとPlanner校正。過去ログを再測定値に置き換えない。

## D17: 文書と品質ツールの入口

- 状態: 設計具体化（未実装）
- 日付: 2026-10-20（JST）
- 決定: PLANが人向けの実行順、SPECが動作規範、READMEが導入/検証、AGENTSが短い作業規則。JS/TSはBiome、型はtsc、Rustはrustfmt/Clippy。root package.jsonはcommonjsを明記し、既存oracle/upstreamの.jsを壊さない。npmパッケージ内は従来どおりtype=moduleとnodeグルーのcommonjs境界を保つ。
- 版の根拠: `npm view @biomejs/biome version --json` の今回出力は `"2.5.15"`。この版を固定し、他の版はSPEC §19.2を維持する。新しいBiomeの実行/設定や製品のbuildが成功したとはまだ報告しない。
- 検証: C0/I0、原本hash、docリンク/節参照、AGENTS 30,000 bytes以下、pack済み配布物の独立検査。

## D18: サーバー配信のみとし、クライアント編集層を除外する

- 状態: 利用者確認済み
- 日付: 2026-10-20（JST）
- 背景: FAQ等の更新は運営者がサーバーへ保存し、索引/差分を配信する。ブラウザ内だけで未保存の文書を検索へ反映するoverlayは不要との確認を得た。
- 決定: D9はサーバー生成の全量索引＋差分セグメント（追加/変更/削除）＋JSON manifestで確定。`.asix`バイナリの形式案を維持する。クライアントの可変セグメント、client_dead、upsert/remove/forget/forgetAll編集APIとそのテスト要件を削除する。旧D9判断4とD12のoverlay部分を置き換える。
- 境界: 初期構築のfromStrings/fromDocs/buildIndex、CLI JSONLのupsert/remove操作名は残す。既存索引へのブラウザ内編集とは別。更新検知/ビルド起動/CDN配置/取得・通知/画面表示は利用側が担当し、全ユーザーへの同時・即時反映は約束しない。
- 検証: SPEC §14.1 U4/M4でサーバーセグメントの差分/置換、旧状態保持、cache整合性、世代/descriptorを検査。旧overlayモデルを現在の更新契約の検証済み証拠にはしない。
- 影響: AGENTS、PLAN、SPEC §0/§9/§11/§12/§14/§15/§17/§20。M0の照合意味論は変更しない。実装・commit・公開はこの判断記録の更新では行っていない。

## D19: 開発環境と公開用の品質設定

2026-10-20（JST）。利用者の「周辺設定はfindrop同様、モダンな構成」という指定を受け、開発用Node 24.22.0、pnpm 12.14.0、製品の型検査TypeScript 7.0.2、Biome 2.5.15、文書のPrettier 3.9.10を採用する。版は実際のregistry／CLI出力で確認した。RustはM0で検証する1.97.0をrust-toolchain.tomlに固定し、同じツールをmiseで二重管理しない。後続の更新は互換検査と一緒に行う。

上流ソースの期待値生成だけはTypeScript 5.9.3をtools/fixturesの依存に隔離する。製品コンパイラを更新してもoracleの意味論を同時に変えない。npm lockfileをpnpm lockfileへ一本化するが、配布検査にはnpm packを使い、Nodeのnpm利用者にも配布可能なことを確認する。

CIはRustジョブとNode 22/24のWASMジョブを定義し、Chromium smokeをNode 24ジョブへ統合する。Actionsは完全なcommit SHAで固定し、最小のread権限で実行する。型検査・整形・lint・tarball検査を必須とする。VS CodeとEditorConfigで保存時整形やGit状態の視認性を揃え、上流コピーと計測原本は整形しない。

公開ファイルには開発者個人の端末パス、秘密情報、非公開ノートへの参照・依存を入れない。許可された著作権表記、上流の著者情報、公開GitHub URLは由来の情報として保持する。WASMにも個人のビルドパスを残さない。

## D20: M1の最小level APIの境界

M1で具体化。matchLevelは既定maxAmbig=3、厳密な整数0〜31を受理する。既存matchの非単調性・32以上の折り返しは維持し、最小levelの探索へ折り返しを持ち込まない。Rust側で1回走査して受理状態の最初のlevelを選ぶ。JS反復は検証oracleとしてのみ使う。型・例外順とRustのResultはAPI仕様に記載し、全fixtureの最初の一致、上限、孤立サロゲート、解放後、確保ゼロを検査する。

## D21: 汎用ScalarWmとWorkspaceの入口

互換Asearchを変更せず、Rustの拡張はScalarWm<W>＋ScalarLimits＋ScalarWorkspace<W>として分離する。BitWordはsealedとし、語容量や記憶量の規則を外部実装で破れないようにする。Idealの初期ε閉包を使い、u32/u64/u128とMultiWord<N>で同じ結果を返す。静的な語幅選択で内側ループのdynamic dispatchを避ける。これは単体照合器であり、後続のコーパス単位Engineではない。

構築時の一時配列は(word, mask)を位置ごとに複製せず、記号と位置を集めて一意な記号だけに完成maskを確保する。長い同一文字パターンで不要な二次容量を要求しない。matcher＋1workspaceの予算を照合前にも確認し、別matcherから借りたworkspaceでも上限をすり抜けない。複数instance/sessionの総計はM2以降の所有者が追跡する。語境界・65,535単位・予算拒否・独立DP・確保ゼロを検査し、速度については未測定とする。

## D22: 単発AsearchのIdealプロファイル

既存compilerとJS casingを共用し、profile="ideal"をFFIの値2へ対応させる。Rust Asearchの初期状態をprefix集合へ切り替え、遷移と終了判定は共用する。互換profileのシフト折り返し・非単調性は維持し、Idealだけを正しい初期ε閉包とする。level32以降はprefixが飽和し、語の開始位置へ巻き戻らない。pattern上限は全profileで31 UTF-16単位のまま。拡張ScalarWmと単発JS APIを混同しない。独立DP、source全fixtureの最小level、孤立サロゲート、Node両版・3ブラウザで検証する。

## D23: コーパス単位のEngine境界

Engine traitはQuery/Workspace/Errorを関連型にし、コーパス全体の最小level配列を返す。最初のScalarEngineは借用text全件を走査し、結果slotと状態を事前予約する。単体ScalarWmをEngineと呼び替えず、後続の転置実装が独自のQuery表現を選べるようにする。key単位の順位・重複排除はM2の索引層へ分離する。借用textの所有容量とadapter/結果の確保容量を区別し、複数instanceの合計予算を追跡していると偽らない。workspace流用、空コーパス、全件level、予算拒否時の旧出力保持と確保ゼロを検査する。

## D24: ja-basic-v1の限定対応表と確保しない変換

ブラウザのICUやPythonの実行時Unicode版に依存せず、Unicode 17.0.0の版固定データから半角カナとかな合成だけを生成する。元ファイルのSHA256を生成器で検査し、Unicode License V3をRust/WASM両配布へ含める。normalize_intoは予約済みUTF-16バッファへ書き、入力以上に拡張しない。合成をかな変換より先に行い、半角濁点だけを対象にする。全NFKC/NFCへの拡張はしない。

項目6の統一先はU+30FC。ASCIIハイフンも対象にして、項目2が全角ハイフンをASCIIへ変えた後でも同じ結果になるよう具体化した。既定では項目6はoffを維持する。全64設定の単位/かな対/混合列の冪等性と、容量不足時の旧出力保持・確保ゼロを検査する。これはM2のRust正規化基盤であり、一括索引の完成を意味しない。

## D25: ASIX v1の圧縮と原文保持

OLEN/OTXTはsegment全体に対する任意の対で、文書単位の「原文なし」を符号化しない。同じ実効文書集合に原文あり/なしを混ぜると、1baseへの圧縮時に原文を捏造するか、既存原文を失う。v1ではlive文書の保持方式が一貫しないcolumnをINVALID_INDEXで採用前に拒否する。死んだ文書と空の削除segmentはこの判定へ含めない。原文なしの索引自体は引き続き対応し、実効集合全体を原文なしへ置換することも可能。混在を表す新しいwire形式はv1へ暗黙に追加しない。

圧縮はcanonicalなlive行を共通encoderへ借用し、別のTransposedEngineを作らない。入力順やsegmentの境界をバイト列へ漏らさず、原文/正規化文をそれぞれ保存し、TOMBを落とす。空の実効集合は標準ビルダと同じ空baseになる。

## D26: 説明の座標と整列

M5の説明は検索用workspaceとは別の、残り総予算内に収まるDP行列を使う。収まらない場合はRESOURCE_LIMITを返し、索引・refsを変更しない。原文を正規化し直してUTF-16の対応区間を作り、呼出側originalがあれば索引の正規化済みtextとの一致を検査する。無ければ保存原文、保存原文も無ければnormalized-utf16を返す。

同点は終端が最も左、始端が最も右を優先する。経路の追加同点は一致・置換・pattern削除・text挿入・無料loopの順で決定する。ハイライトはリテラル/置換/有料挿入で消費したtextを含み、無料のInclude/WildcardSpace部分を含まない。pattern削除はtext区間を持たない。隣接区間は結合する。終端は最後に全候補から選び、途中のprefixでは右端の始端を保つ。途中で早い終端だけを選ぶと、内部ワイルドカードの後で最短区間を失うためである。

## D27: IntentIndex v1の構文と資源境界

M6のparserはUTF-16入力を扱い、リテラル・グループ・選択・1回の後置`?`・次のcode pointのエスケープを受理する。空式/空グループ/空選択肢はε。閉じ忘れ・対応しない`)`・単独/重複`?`・末尾の`\`・エスケープしていない`*`/`+`はINVALID_INPUT。非BMPのリテラルに付いた`?`はサロゲート対全体を省略する。重複keyは文書索引と同様に拒否し、選択肢は1つの式へまとめる。

固定の安全上限は、グループ深さ64、式あたり65,535 UTF-16単位、raw DAG全体262,144 node/524,288 edge、変換後全体524,288 node/1,048,576 edge、1 nodeあたり128種類の保留記号状態。ResourceLimitsの入力/text数/units/slots/working/pattern予算も合わせて適用し、超過時は黙って打ち切らずRESOURCE_LIMIT。閉路を導入しない。

構文解析を正規化より先に行い、全角の括弧などを構文として誤解しない。正規化とUTF-16からcode pointへの変換は辺をまたぐ保留状態を持つ変換として行う。`(ｶ|ﾊ)ﾞ`や、グループ境界をまたぐサロゲート対も、選ばれた全文字列をまとめて正規化/記号化した結果に合わせる。中間node/stateの増加も同じ上限・予算へ含める。

照合はDAGとpatternの積上の最小費用で計算し、展開文字列を列挙しない。同じkeyの並列式やクライアント更新層は追加しない。最良展開が複数ならparser由来の決定的な辺順で選び、その原文展開にM5の整列規則を適用する。M6のDAG経路では残予算内の明示的な作業確保を許し、準備済みbulk検索の確保ゼロ保証と区別する。

M6の現行照合には状態語OR方式に代えて、DAG×patternの最小費用行列を採用した。展開数に依存せず、正規化の保留状態と原文経路の復元を同じグラフ上で検証できる。計算量はO(edges×pattern記号数)で、状態語への置換は同値性と計測を必要とする別の最適化として扱う。

## D28: M7の採否とスカラー経路の改善

2026-10-21（JST）。M7は任意の高速化を評価し、有効性を確認できるものだけを残す段階とする。今回採用するのは、スカラー経路での完全候補収集と結果抽出の統合、literal classの共通コンパイル、予算を超える候補集合の破棄と全量経路への復帰である。top-N外・削除済みの候補も含める意味論、独立oracle、資源上限と全既存テストを維持する。

同じ最終成果物でblocked/autoをAB/BA計測し、実コーパスの逐次入力と1〜8segmentの検索・反復lifecycleを検証した。Nodeでは逐次入力p95の改善を確認したが、中央値には悪化もあり、autoを全負荷で速いとは主張しない。Firefox/WebKitの0ms標本を捨てたり、分解能以下の値から速度比を作ったりしない。

SIMD・Myers・OSA・NodeAsearch・別のJS高速経路は、同じ製品境界での互換性と再現する優位性の証拠を今回用意していないため採用しない。これらの製品prototypeや比較計測を実行済みとは扱わず、基本命令WASMだけを配布する。SIMDの機能検出・二成果物・fallbackの受入条件は、将来採用する場合に適用する。性能予測による不利の断定もしない。非対応engine指定は明示的に拒否する。

本体M0〜M7のローカル受入と、別repoの固定版Workerデモを検証した。GitHub hosted CI・Pages配信、端末実機、全入力の安全性、全用途の性能はこの受入の保証範囲に含めない。
