# 性能検証ハーネス

M1で実装中。NodeとPlaywrightのChromium/Firefox/WebKitで、上流asearch.tsの公開JSとRust/WASMラッパの固定level照合を比較する。profileはasearch.ts-lib、Include、level 3。これは一括索引や逐次入力の性能ではない。

```sh
mise exec -- pnpm build
mise exec -- pnpm test:bench
mise exec -- pnpm bench:node OUTPUT_JSON 10000
mise exec -- pnpm exec playwright install chromium firefox webkit
mise exec -- pnpm bench:browser OUTPUT_BROWSER_JSON 10000
mise exec -- pnpm bench:compare BASELINE_DIST CANDIDATE_DIST OUTPUT_JSON BASELINE_COMMIT CANDIDATE_COMMIT 10000
mise exec -- pnpm bench:native OUTPUT_NATIVE_JSON 10000
```

OUTPUT_JSONは測定ログの保存先。UNITSは10,000〜10,000,000で、seedは31。計測中に他のビルドやテストを並行実行しない。生成器は3,000個のBMP記号、Zipf指数1.05、text長10〜60を使う。

計測前にpatternごとに全text・level 0〜3の真偽が一致することを確認する。patternは3/7/15/31記号。構築時間を除外し、各sampleは全候補へのmatchの平均時間。タイマー分解能を超えるようバッチ反復数を自動調整する。5 paired runs、各30標本をAB/BA交互に測り、runごとの中央値比をbootstrapする。出力のp50/p95は**バッチ平均の分布**で、個別の打鍵レイテンシではない。

raw標本、seed、コーパス・上流・WASM・JSラッパ・lockfileのSHA256、HEAD、日時UTC、Node/V8/Rust/OS/CPUをJSONへ保存する。Nodeの上流はVM context、ブラウザの上流は専用のCommonJS/verbose shimを渡したFunctionで実行し、製品はNodeエントリから呼ぶ。比較はこの呼出し経路を含む。コアだけの性能とは区別する。

各runnerは未コミット変更の有無もbooleanで保存する。dirtyの場合、HEADだけでは実測ソースを特定できないので、artifact/runnerのhashと測定時の差分を合わせて記録する。端末の変更ファイル名はJSONへ保存しない。

統計モジュールはノイズのある架空標本、既知の10%超回帰、独立再確認の有無、固定seedによる再現性をテストする。単一の5run組だけでは回帰を確定しない。ブラウザは3エンジンを順番に測定する。

Node/ブラウザの`replicaResults`は、verbose/process呼出しを持たない素直なJS移植との固定level比較。同梱上流のコードは変更せず、レプリカを全fixtureで別に検証する。これはベンチ専用で、JS製品経路ではない。`extensions.minimumLevels`はJSでmatchを0..K回呼ぶ最小levelと、製品matchLevelの1回呼出しを比較する。各textの最小levelを揃え、key順位・top-Nを含まない。両者とも同じタイマー・コーパス・AB/BA方式を使う。

`extensions.construction`は1回ごとにconstructorを呼び、freeがある実装はその場で解放する。最後のインスタンスを観測可能に保持して、未使用な構築処理を測らないようにする。上流JSのGCは遅延するため、解放費用が同じ範囲に入るという保証は無く、constructor/freeの費用を知る参考値とする。生存数・GCを制御した同等の解放ベンチとして速度優位性を主張しない。この2カテゴリの分位点はbatchMeanOperationMsという名前で、固定level照合と区別する。いずれも個別打鍵の分位点ではない。

`bench:compare`は変更前後のビルド済みdistを同じNodeプロセスへ読み込み、同じコーパスをAB/BAで比較する。独立した過去のJSON同士を速度比にしない。BASELINE_COMMITとCANDIDATE_COMMITには、それぞれのdistを生成した40桁のcommit IDを指定する。ビルドは同じツールチェーン・依存版で行い、実際の版を測定記録へ併記する。呼出側が指定するcommitラベルと成果物の対応は呼出側が確認する。runnerは両方のWASM・JSラッパ・生成グルーのSHA256、コーパスとハーネスのSHA256を記録する。記録に端末のファイルパスは含めない。

4パターンのいずれかで回帰候補が出たら、もう1組の5runを全パターンについて測り、個別に再確認する。p50/p95/p99はいずれも150標本のバッチ平均分布であり、個別操作の分位点と区別する。各patternの計測終了時と異常終了時には、両方のハンドルをfree可能なら解放する。構築・解放の時間はこの固定level計測には含めない。

`bench:native`は同じworkloadをCLIへ渡す。RustのAsearch Idealによる各textのmatch_levelとScalarEngine<u32>の全件level走査を、全textのlevel一致を検査してから比較する。normalizerなし、Rust casing、Include、K=3。どちらもネイティブコアであり、JSとの速度比ではない。上位key順位付けも含まない。std::time::Instantを使い、10回ウォームアップ、5run×30標本、AB/BA、最小4msを目指す共通反復数で測る（上限1,024）。集計はJSと同じ統計モジュールへ渡す。raw、コーパス、実行ファイル・Cargo.lock・runner・集計のSHA256、commit、Rust/Node/OS/CPUを保存する。ビルドは計測前に完了する。比較する2実装の比率であり、単一の実測だけを変更前後の回帰判定として扱わない。

CLIの直接利用は`asearch-cli bench INPUT_JSON OUTPUT_JSON`。入力はworkloadと同じseed/units/texts/patternsで、JSONは64 MiB、pattern数は1〜64、宣言unitsと実データが一致する10^4〜10^7 UTF-16単位を受理する。native Asearchとの比較なのでpatternは31 UTF-16単位以内。長いパターン単体の正しさはRustテストで別に検査する。

## 一括索引の逐次入力

```sh
mise exec -- pnpm bench:session INPUT_ASIX OUTPUT_JSON node
mise exec -- pnpm bench:session INPUT_ASIX OUTPUT_JSON browser
```

`INPUT_ASIX`は呼出側が用意したASIX base。データセットを本体へ組み込まない。browserはChromium/Firefox/WebKitを順番に実行する。索引のSHA256をブラウザでも照合し、localhostで指定ファイルだけを追加配信する。

同じ索引を共有するblocked sessionとauto sessionを、同じ既定ポリシー・countTotal=true・limit=30で比較する。4系列21打鍵（延長・後退・末尾置換・半角濁点・空入力）を10回warmupし、5run×50roundをAB/BA交互に実行する。各updateを個別に計時し、結果のids/levels/orders/厳密totalsは計時区間外で照合する。reset・構築・ロード・描画は含めない。raw、engine使用件数、標本数、p50/p95/p99、環境と成果物hashを保存する。Firefox等の粗い時計による0ms標本は捨てずに記録し、0を分母にした速度比は計算しない。普遍的な閾値や端末の保証には使わない。

## 複数セグメントの計測

```sh
mise exec -- pnpm bench:segments OUTPUT_JSON node
mise exec -- pnpm bench:segments OUTPUT_JSON browser
```

固定の合成1万文書・2万textを1〜8segmentへ分割し、同じフル索引とAB/BA順で照合します。各segment数・各経路1,300回（13入力×5run×20round）のsearch呼出時間を保存し、ids/levels/orders/exact totalsとcompaction bytesを照合します。ロード・圧縮・更新は新しいrootで3回のウォームアップ後に各30回計測します。取得元はメモリ上のbytesで、HTTP通信時間ではありません。先頭の単独観測値は診断用で、30標本の分布とは区別します。ロードと更新にはexpectedのSHA-384検査を含みます。8segment後の更新は圧縮base＋deltaへの全量置換なので、他のappendと処理量が異なります。

WASM memoryのbyteLengthは縮まないページ数の観測です。同じinstanceで先行シナリオも実行するため、現在のlive allocationや厳密な操作別ピークと同一視しません。合成データの結果を実データの検索品質・端末性能へ一般化しません。2026-10-21のローカル実行では、segment追加の検索オーバーヘッドと全量置換の高いロード費用を確認しました。小さなdeltaと定期圧縮を組み合わせる方針を維持し、既定の圧縮目安を全用途の最適閾値として保証しません。
