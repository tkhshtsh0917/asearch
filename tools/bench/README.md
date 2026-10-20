# 性能検証ハーネス

NodeとPlaywrightのChromium/Firefox/WebKitで、上流asearch.tsの公開JSとRust/WASMラッパの固定level照合を比較する。profileはasearch.ts-lib、Include、level 3。これは一括索引や逐次入力の性能ではない。

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
