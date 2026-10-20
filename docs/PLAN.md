# 開発計画 — asearchのRust/WASM化

2026-10-20（JST）。この計画は実行順・受入ゲートを定める。動作の規範は[仕様索引](spec/README.md)、由来は[DECISIONS](DECISIONS.md)、環境準備は[README](../README.md)。AGENTSはエージェント向け作業規則である。

## 1. 目的と完成像

Rustライブラリを主役に、同じ照合コアをWASMでブラウザのメインスレッド/Workerへ配布する。短い入力で大量のFAQ・言い換えを打鍵ごとに探す用途を主対象とする。公開名はcrate `asearch-rs`、npm `asearch-wasm`。

- M0はasearch.tsの意味論・型・既知の癖を再現し、以後の改善の互換基準にする。意図的逸脱はD10に限定する。
- M1で完成品の境界コストを含む比較基盤を作り、M2で一括検索、M3で逐次入力を高速化する。
- M4で事前構築した索引と差分のロード・更新を支え、M5で説明、M6で非展開の意図表現を加える。M7は計測で有効な場合だけ採る。
- 「最速」は目標。主張できるのは同じ結果・同じ環境・同じ負荷で比較した範囲だけ。既存のC代理実装の数値をRust製品の性能として使わない。

UI、DOM/描画、IMEイベント制御、形態素解析/読み/同義語の生成、CDN設定・manifest取得/監視、サーバー検索API、実データの収集は範囲外。外部で作った言い換えは同じkeyのtextsへ渡せる。境界はSPEC §0。

## 2. 現在の準備状態とゲート

| 項目       | 状態と扱い                                                                                                                                |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| D1〜D8     | 既定profile/ポリシー/名前/正規化/mode/順序/ライセンス/スコープを維持                                                                      |
| D9         | 2026-10-20確認済み。サーバー生成のbase/delta/tombstoneのみを検索。ブラウザ内の編集層は対象外（D18）                                       |
| D10        | M0互換境界の既定提案。変更要求がなければこれで実装する                                                                                    |
| D11〜D17   | 今回の計画整備で曖昧さを具体化。初期化契約はM0で検査。索引・資源・性能の新規規範は後続段階で検証                                          |
| 著作権表記 | `Copyright (c) 2026 Toshinori Takahashi`（確認済み）。移植元の表示は別途保持する                                                          |
| GitHub/CI  | 公開先はhttps://github.com/tkhshtsh0917/asearch。CI定義・ローカル実行とhosted成功を分けて報告する                                         |
| ツール     | SPEC §19.2の版を使用。Nodeはmise、Rustはrust-toolchain.tomlを正本とし、同じRust版を二重管理しない。別プロジェクトのglobal設定を変更しない |

M0とM1のローカル受入を完了した。M0は両profile全fixture・配布物・初期化、M1はmatchLevel/free/dispose・Ideal・汎用ScalarWm/予約Workspace・コーパスEngine・CLI検証/計測を実装済み。P1の意味論一致、AB/BA、ノイズ/合成回帰、集計再現性を検証し、Node・Chromium・Firefox・WebKitで互換API/JS移植/最小level/構築の計測、ネイティブコアの全件level走査、変更前後のWASM比較を行った。測定範囲と再現手順は[ハーネス](../tools/bench/README.md)。共有CIの速度閾値や全用途での優位性は保証しない。

M2はRustのversion付き正規化、検証済み固定/ByLengthポリシー、資源予算付きQueryWorkspace、ブロック転置のTransposedEngine、単一文書索引AsearchIndexを実装した。from_docsは原文・正規化済みUTF-16と派生構造を所有し、keyごとの最良text、(level,order,key)順のtop-N、limitに依存しないtotalsを返す。ASIX v1 baseの保存/ロード・XXH3検証もRustに実装し、切り詰め・構造不正・原文整合性・決定的変異4,000件を検査した。CLI build/searchを追加し、デモの質問文1万件で索引構築と検索を確認した。JS一括API（全profile、JS casing、コピー結果）・同じ文書集合でのnative/WASMバイト一致を検証した。fromStreamと所有輸送バッファのピーク予算も実装した。10^7単位/365,091文書でNode/3ブラウザの検索・ロードを計測し、この規模でもnative/WASMバイト一致を確認した。libFuzzer/ASanで6,448,126入力を31秒間検査し、panic/asan検出なし（CI jobは未実行）。JS locale casingを全bulk profileへ接続し、単発APIとの最小level一致・Turkish casing・サロゲート境界・解放再入も検証した。空text多数の境界予算と同時workspace登録/解放も検証した。M3はbounded row checkpointとJS SearchSession(update/reset/free)を実装し、正規化後の接頭辞・K変化・親解放・同時容量を検証中。カスタム関数ポリシーもnormalized contextで評価する。完全候補のNarrowingと保守的な費用判定も実装した。残る受入は3ブラウザの打鍵計測とengine切替の校正・長い編集系列の追加検証である。M4はmanifestの厳密検証と純粋なdiffManifest（系列/世代/共有descriptor/append/replace判定）を実装中。expected descriptorのbytes/texts/id/SHA-384検証をnative/WASMの共通Rust実装へ接続した。Rustのfrom_delta・TOMBのcanonical保存/ロードを実装した。複数segment採用・CLI/WASM deltaビルダ・compactは未実装。M5以降も未実装。§5.2のマスク参照表や§12.2の境界バッファは後続の性能改善候補であり、現在の互換経路はソート済みmask表とUTF-16コピーを使用する。GitHub上のCIは未確認で、push／npm公開は行っていない。

## 3. 実装順と完了条件

下表の受入IDは[SPEC §14.1](spec/validation.md#section-14-1)に対応する。各段階で直前までのテストを維持する。機能と同じ変更でテストを作り、DoD未達を完了にしない。見積もり日数・未測定の性能値は置かない。

| 段階              | 作るもの                                                                                                      | 完了判定と次へ渡す証拠                                                                                                                                                                     |
| ----------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| M0 互換移植       | u32コア、薄いwasm-bindgen境界、JSコンパイル、web/Node初期化、型、fixture、CI、README/NOTICE                   | C0/I0。両profile全fixture、上流例・36assertion、型互換、確保検査、pack済み配布物、Chromium/Worker、初期化失敗/再試行。全必須コマンドと未検証事項を報告し、成果物をinitial commitにまとめる |
| M1 計測と互換拡張 | Node/3ブラウザ計測、matchLevel、free/dispose、ScalarWm語幅一般化、Engine、必要なCLI bench/fixture検証コマンド | P1とM0回帰。JS境界/コア/全量の意味を分けたベースライン、集計再現性。31記号制限を維持する互換APIと資源予算を持つ拡張経路を分ける                                                            |
| M2 一括と単一索引 | AsearchIndex、転置blocked、正規化、既定ByLength、top-N/totals、単一segmentのbuild/検証/stream load            | Q2/R2/F2、全engine==独立oracle、native/WASM決定性、format fuzz。主対象規模と10^7単位で検索/ロード/容量を報告し、暫定資源予算を校正する                                                     |
| M3 逐次入力       | SearchSession、Incremental、Narrowing、Planner、カスタムAmbigPolicy、Worker例                                 | S3、正規化後LCPとK増加/復帰条件、cache枯渇fallback。3ブラウザの打鍵p95と標本数、engine切替閾値の根拠                                                                                       |
| M4 更新と配信形式 | 複数segment、delta/compact、manifest差分、buildIndex/mergeResults                                             | U4/M4と既存property。失敗しても旧状態保持、セッション付き更新、世代/系列/整合性検査、native/WASM/compactionのバイト一致。1〜8segmentと更新/配信/ピークの計測                               |
| M5 説明           | explain、原文位置への写像、同点規則                                                                           | E5、半角濁点等の位置と参照寿命。原文が無い場合はnormalized-utf16を返し、原文位置と区別する（§9/§12）                                                                                       |
| M6 意図表現       | parser/DAG、エントリ絞り込み、最良展開                                                                        | D6、小規模全展開oracleと指数的選択肢の非展開。parser/ノード/辺の予算と曖昧な文法入力を着手時に固定                                                                                         |
| M7 任意の高速化   | SIMD/Myers/OSA/NodeAsearch/JS高速経路                                                                         | O7。同値性と機能検出を先に確認。再現する優位性が無いものをautoの既定に入れない。実験として残すか削除する                                                                                   |

M2のByLength descriptorは既定計算に必要な範囲を先に実装し、M3でPlannerとカスタム関数へ広げる。未到達APIのstubをM0に並べない。最終形のディレクトリ一覧を最初から空ファイル群として作らない。

## 4. 全段階に共通する受入基準

1. 正しさ: upstream fixture、独立DP、複数engineの差分を使い分ける。実装者と同じ最適化で書いたコードだけを正解にしない。
2. 安全性: 入力・数量・容量を確保前に検査し、失敗を型付きエラーへする。更新はcandidateで検証してatomic採用。失敗時は旧索引/世代を保持する。
3. 所有権: Rust coreはunsafe禁止。借用ビューとコピーの寿命、解放済みhandle、更新中の操作を明文化し、境界テストを持つ。
4. 再現性: lockfile、同梱upstream hash、fixture seed/locale、native/WASM一致、npm pack後の別ディレクトリ検証。新規JSはBiome、型はtsc、Rustはrustfmt/Clippy。
5. 性能: 共有CIの時間閾値は必須にせず、SPEC §15.1.1のpaired計測と再測定で判定する。暫定予算・観測・保証を区別する。
6. 報告: 実装した機能/残件、実行コマンド・件数、逸脱、未検証事項、性能条件、次のゲート。失敗を実行成功の件数へ含めない。

## 5. 保留事項と設計変更の扱い

- 本体repoはRustコア/CLI/WASM/薄いTS APIと検証に絞る。GitHub Pages用UI・データセット・配信設定は別repo [asearch-demo](https://github.com/tkhshtsh0917/asearch-demo)へ配置する。本体からデモへの依存は持たず、デモは検証済みの版固定成果物を消費する。repoの新設とcloneは完了し、UI/Pages接続は開発中。デモはAutoWikiQAの質問文を対象にし、データ取得・帰属表示・CC BY-SA 4.0の管理はデモ側で行う。本体はそのデータに依存しない。
- 著作権表記とGitHub公開先は確認済み。hosted CIはpush後に確認する。
- D9確認は完了。サーバー配信のみを対象とし、クライアント編集APIは実装しない。各段階の受入条件は引き続き満たす必要がある。
- M1の拡張Rust APIの入口とWorkspace予約の型はScalarWm/ScalarLimits/ScalarWorkspaceとして具体化済み（D21）。後続段階で具体化: M6のparser資源上限。該当段階着手時に型/数値/ケースを確定し、その前の段階に実装を混ぜない。
- 性能未達: 正しさや互換の基準を緩めず、対象ケース・予算・engine選択を計測して見直す。製品が未達なら未達と報告する。
- API/format/normalizer公開後: 互換規則に従ってversionを上げる。過去の観測ログや上流コピーを新仕様へ書き換えない。
