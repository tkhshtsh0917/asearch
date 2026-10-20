# レイアウトと照合エンジン

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-3"></a>

## 3. 記号化とコーパスのレイアウト

<a id="section-3-1"></a>

### 3.1 記号化

- プロファイルの記号単位で各 text を分解し、出現する全記号に密な ID を振る（u16、65,535 種を超えたら u32）。ID 0 は UNKNOWN（どの EQ にも属さない）。
- 記号 ID と記号表は**セグメントごとにロード時に作る**（索引ファイルには正規化済みの UTF-16 原文だけを保存する。§11.1）。ID は版をまたいで安定しないので、ファイルにも API にも出さない。
- 記号表: コード値の昇順配列。クエリ側だけが二分探索で引く（pattern は短いので十分）。
- pattern 側: 各位置のキー集合（最大 3、正規化で増えることがある）を記号 ID の集合に写像する。未知の記号は空集合（一致不能だが位置は占める）。

<a id="section-3-2"></a>

### 3.2 転置レイアウト（ブロック整列）

- 各 text k の直前に境界スロット b_k を 1 つ置く: `[b_0][t_0 …][b_1][t_1 …] …`
- 64 スロット = 1 語。B 語（既定 512 語 = 32,768 スロット）を 1 ブロックとする。
- **text はブロックを跨がない。** 入りきらない場合は残りを「死スロット」で埋めて次のブロックから始める。すべてのブロックは境界スロットで始まる。
  - これにより、(a) シフトのブロック間キャリーは先頭の境界上書きで捨てられ、(b) fill の加算キャリーも死スロットで止まる。**ブロックは完全に独立**になり、並列化、SIMD レーンへの割当、Worker 分割、追記更新がすべて自明になる。
  - B より長い text は、そのブロックだけ可変長に広げて良い（まれ）。
- 語配列のマスク: `SEP`（境界スロット）、`TXT`（text スロット）、`END`（各 text の最終スロット。空 text は b_k 自身）。死スロットはすべて 0。末尾に全 0 のガード語を置く。
- rank: `endcum[w]` = 語 w より前の END の個数。text ID = `endcum[w] + popcount(END[w] & ((1 << b) − 1))`。
- text の並び = `(order, key, key 内の text の並び)` の昇順（§11.3）。key と order の配列を別に持つ。
- ブロック幅 B は実行時のパラメータで、索引ファイルには保存しない（端末ごとに索引を作り直さずに調整できる）。

<a id="section-3-3"></a>

### 3.3 EQ コンテナ（Roaring 風）

- 記号ごと・65,536 スロットのチャンクごとに、出現数が 4,096 を超えればビットマップ（1,024 語）、以下なら u16 オフセット配列で持つ。
- ブロックの EQ 構築: ビットマップは memcpy、配列はスキャッタ。1 クエリ中は (記号クラス, ブロック) 単位でキャッシュする（同じ文字が pattern に複数回出る場合）。

---

<a id="section-4"></a>

## 4. 転置ビット並列 DP（TransposedBlocked、既定の全量エンジン）

<a id="section-4-1"></a>

### 4.1 考え方

通常の Bitap はビット＝pattern 位置、ループ＝text の記号。本エンジンは**ビット＝コーパスのスロット、ループ＝pattern 位置（行）**とする。語幅 64 を常に使い切れるので、m が小さいほど有利になる（Allison–Dix の LCS と同じく、どちらの列をビットに載せるかは自由）。

`S[i][e] = { スロット j : C[i][j] ≤ e }`（Ideal なので単調: `S[i][e] ⊆ S[i][e+1]`）。

<a id="section-4-2"></a>

### 4.2 漸化式（検証済み: `ref.py` / `verify.py`）

`sh(x)[j] = x[j−1]`（スロット番号が増える方向へ 1 ビット、語間キャリー付き）。

```text
行 0   : X = (e > 0) ? sh(S[0][e-1]) : 0
行 i≥1 : X =  (sh(S[i-1][e]) & EQ_i)                                        // 一致
           | (e > 0 ? sh(S[i-1][e-1]) | S[i-1][e-1] | sh(S[i][e-1]) : 0)    // 置換 | 削除 | 挿入
           | (OSA && i ≥ 2 && e > 0 ? sh(sh(S[i-2][e-1])) & sh(EQ_i) & EQ_{i-1} : 0)   // §4.6
共通   : X = (X & ~SEP) | (B(i,e) ? SEP : 0)          // 境界列 = 初期状態
         S[i][e] = (i ∈ LOOPS) ? fill(X) : X
受理   : text k は level e で一致 ⇔ S[m][e] の END_k ビットが 1
```

- level は e の昇順に計算する（挿入項が同じ行の e−1 を使う）。
- B(i,e) を §2.6 の表で差し替えるだけで、`AsearchTs`/`AsearchTsLib`/`NodeAsearch`/`Ideal` それぞれの状態集合と**ビット単位で一致**する（検証済み: 2 シード計 20,071 ペア × level 0..5 で不一致 0）。一括 API は §2.8 により Ideal を使う。

<a id="section-4-3"></a>

### 4.3 自己ループの fill（キャリー・トリック）

```text
y = (x & TXT) | (sh(x & SEP) & TXT)        // 境界スロットの状態を text 先頭へ種まき
f = (((y + TXT) ^ TXT) & TXT) | y          // 多語加算: 各 text 区間で最下位の立ちビットから区間末尾までを 1 にする
fill(x) = f | (x & SEP)
```

- 各 text 区間の直後（次の境界、死スロット、ガード）は TXT = 0 なので、キャリーはそこで吸収される。
- 語をまたぐ加算キャリーは逐次処理で良い（スカラ u64）。

<a id="section-4-4"></a>

### 4.4 抽出（順序付き top-N）

- Ideal は単調なので、level e で新たに一致する text = `A_e & END & ~A_{e−1}`（`A_e = S[m][e]`）。
- e = 0..K の順に、各 level 内はスロット昇順（＝ text 順）に立ちビットを走査し、rank で text ID に変える。これで (level, text 順) の並びが自然に得られ、**ソートは不要**。
- `limit` 件に達したら打ち切って良い（同じ level 内も text 順なので、先頭の limit 件が上位）。結果は key 単位で、同じ key の 2 件目以降は捨てる（既出 key の集合は事前確保した開番地法の表で持ち、クエリごとに touched リストで消去する）。
- 削除済み（tombstone）の text は `LIVE_END = END & ~dead` で除く（§20.3）。複数セグメントでは、この抽出をセグメントごとのカーソルとして遅延的に進め、k-way マージする。
- `countTotal: true` のとき `totals[e]` は、実効の文書集合のうち **最小 level がちょうど e の異なる key の数**（e=0..K）とする。総件数はその和。`limit` は総数を変えない。同じ key の最良 text だけを数え、`LIVE_END` と全セグメントの重複排除を適用する。
- この計数は top-N の早期終了とは独立に全候補を走査する。単純な popcount は text 数であり、key ごとに複数 text がある場合の総件数には使わない。全levelの累積集合を足してもならない。`countTotal: false` では `totals` を返さず、早期終了できる。ゼロ件は `length=0`、要求した `totals` は K+1 個の 0。計数の追加費用もベンチで報告する。

<a id="section-4-5"></a>

### 4.5 ブロック実行

- 1 ブロックの全行を、L1/L2 に収まる 2 本のローリングバッファ（各 (K+1)×B 語）で計算し、最終行だけを抽出に回す。
- ブロック早期終了: `i > i_B`（全 e で B(i,e) が偽になる最小行。Ideal と AsearchTs では K）かつ `S[i][K]` がブロック内で全 0 なら、以降の行もすべて 0 なので、そのブロックには一致がない。Ideal は単調なので level K だけ見れば良い。OSA 有効時は「連続 2 行が全 0」を条件にする。
- K は 0..=3 をコンパイル時定数として単相化し、4..=7 は動的ループにする。K = 0 は Shift-And（完全部分一致）に退化し、とても安い。

Rustの`TransposedEngine::new(&[&[u16]], TransposedOptions, ResourceLimits)`は正規化から派生構造を作り、`workspace(query, max_ambig)`の予約後に`Engine::search`でtext順の最小levelを返す。`QueryWorkspace`とnormalizer/profileが異なる場合は拒否する。levelの初期状態はすべてIdealで、AsearchTsLibはtextのUTF-16記号化だけを切り替える。ネイティブRustの大小文字候補は単一scalarへの変換だけで、JSのlocale casingを保証するAPIではない。

ホストのcasing候補を使う場合は`search_with_casing`へwrapped patternの各code pointに対応する(original,lower,upper)のu32 tripleを渡す。複数記号展開の不一致候補はu32::MAXで表す。元記号・個数・範囲を検証してからreserved workspaceを書き換え、照合中は確保しない。AsearchIndexも同じ入口を持ち、JS/WASM層が正規化後のホストlocale casingに利用する。ネイティブの通常searchはRust casingのまま。

EQは密な記号IDをコード値の二分探索で引き、チャンクごとに配列/ビットマップを選ぶ。クエリ中のEQキャッシュはブロック単位の8行循環キャッシュで、追い出し時は再構築する。保持量はpatternの異なる文字数に比例して増えない。ブロック間のcarry・padding・guard、拡張長ブロック、空textを扱う。K=0..3は単相化し、4..7は動的処理する。

この単体エンジンは原文やkeyを保持しない。借用入力は`maxTotalBytes`で事前検査し、正規化後の単位数・text数・guardを含むスロット数を制限する。構築時の一時配列と派生配列は確保前に合算し、検索前はengine+query+渡されたworkspaceの容量を検査する。異なるengineのworkspaceも出力長・ブロック容量・予算を再検査してから書き込み、失敗時は以前の出力を保持する。複数workspace/sessionのinstance合計は後続の所有indexが管理する。速度・大規模容量の受入はM2の後続ゲートであり、実装だけを性能達成とはしない。

単一文書索引`AsearchIndex`は原文・正規化済みtext・文書表と転置エンジンを所有する。from_docsは重複keyとtextなし文書を拒否し、(order,key,text順)に配置する。IndexWorkspaceはtextごとのlevelに加え、文書ごとの最良text・結果・totalsを予約する。検索は予約後に確保せず、同点textでは文書内の先頭を選ぶ。現実装は全textのDP後に文書の最小levelを集約し、levelごとに配置順へ取り出す。keyのhash集合や結果sortは使わないが、countTotal=falseでもDP自体のtop-N早期終了はまだ行わない。totalsは最小levelの文書数である。index+query+渡されたworkspaceの容量を合算して検査する。IndexWorkspaceは同じindexの共有予約台帳へquery+workspace容量を登録し、同時保持の合計を確保前に検査する。Dropで予約を戻し、別indexへの流用を拒否する。同じqueryを複数workspaceで共有した場合も各予約に数える保守的な方式である。

<a id="section-4-6"></a>

### 4.6 Damerau（OSA）オプション

- 隣接転置（ab ↔ ba）を 1 誤りとする Optimal String Alignment。漸化式は §4.2 の OSA 項。検証済み（素朴な OSA DP と 12,000 ペアで一致）。
- asearch 互換ではないので既定は無効。逐次エンジンにも入れる場合は、同じ素朴 DP で差分検証する。

<a id="section-4-7"></a>

### 4.7 増分行キャッシュ（TransposedIncremental、SearchSession の密な段階の既定）

- pattern が 1 記号伸びたら、キャッシュ済みの前行から新しい 1 行（全 level）だけを計算する。WildcardSpace で空白が追加されたら、最終行に fill を適用した版を新しいキャッシュ項目として積む（空白の削除で元に戻せるように）。検証済み: `verify_ext.py`（全量計算と不一致 0）。
- **受理位置の自己ループ（Include や末尾空白、`wrap`）は保存行に入れず、抽出時だけ適用する**（次の記号で受理位置が動くため）。
- 行キャッシュは pattern 接頭辞をキーにしたスタック。Backspace はポップ、IME の末尾置換は LCP までポップしてから積み直す。
- メモリ: 1 行 = (K_cache + 1) × 総スロット / 8 バイト。上限（既定 32 MiB、設定可）を超えたら古い行から捨て、必要になったらブロック化全量で再計算する。K が打鍵途中で増えるポリシー（byLength）では、K_cache = そのポリシーの最大 K で計算しておくと再計算が要らない（1 行の費用は (K_cache+1)/(K+1) 倍）。どちらを選ぶかは Planner の費用モデルで決める。
- 抽出は上位 N 件で早期終了し、最終行に一致が 1 つもないことを先に安価に判定できるならそれを使う（全 level の fill を省く）。

Rustの`IndexWorkspace::enable_incremental(index, query, max_bytes)`は全コーパス行のbounded checkpointを有効にする。正規化・ホストcasingを反映したEQクラスと内部wildcard loopの共通接頭辞で再開点を選び、末尾の受理行はcheckpointへ保存しない。容量はポリシー上限まで予約するが、計算・checkpointコピーは現在のKに限定する。Kが変わったらcheckpointを無効化して再計算する。literalが0〜1個の場合は再利用できる内部接頭辞が無いためblockedを使う。古いcheckpointは循環上書きし、必要な行が無ければ行0から再計算する。1 checkpointとscratchが予算に入らない場合はblockedへ戻る。容量は共有workspace予約へ加算し、stats.reused_rowsは実際に省略した行数を返す。これはM3のコア実装であり、Planner・Narrowing・JS SearchSessionの受入完了を意味しない。

---

<a id="section-5"></a>

## 5. ScalarWm（逐次エンジン）

<a id="section-5-1"></a>

### 5.1 役割

1. compat `Asearch.match` の実体（asearch.ts の u32 レプリカ）。
2. Narrowing（§6）の検証器。
3. すべてのエンジンのテストオラクル。

<a id="section-5-2"></a>

### 5.2 実装

- 語の型を trait `BitWord`（u32 / u64 / u128）と `MultiWord<N>` でジェネリックにする。compat 経路は u32 で、JS の `>>>` とシフト量 mod 32 を厳密に再現する（Q4）。
- 位置ビットの向き（MSB 先頭／LSB 先頭）は実装の自由。ただし compat の `masks` プロパティは JS 側で asearch.ts と同じ式で計算して公開する（§12.2）。
- 文字 → マスク表:
  - 一括 API: 記号 ID を添字とする密な表を、touched リストで消去する（クエリごとに O(m)）。
  - compat の単発 `match`: **世代スタンプ付きの直接表**（BMP 65,536 エントリ × (stamp, mask)。pattern 変更時は stamp を 1 進めて O(1) で消去。非 BMP は小さな開番地法の表）。
  - やってはいけない例: node-asearch はパターンごとに 65,536 要素を初期化する。takker99/rust-asearch はパターンごとに 256KB の配列を生成し、非 BMP の記号で配列外参照により panic する。

<a id="section-5-3"></a>

### 5.3 検証方針

Rust/WASM製品の性能は§15に従いM1で計測する。

### M1のRust単体拡張

`ScalarWm<W>::new(pattern, mode, ScalarLimits)`はcode pointのIdeal意味論をコンパイルする。`BitWord`はsealedで、u32/u64/u128/MultiWord<N>を実装する。状態位置0が開始、位置mが受理であり、m+1ビットを必要とする。容量不足はWordCapacityエラー。上限いっぱいの65,535記号にはMultiWord<1024>を指定できる。compat Asearchの公開maskと初期状態は変更しない。

`workspace(max_ambig: u16)`はK+1個の状態語をfallibleに予約し、`match_level(text, &mut workspace)`はResult<Option<u16>, ScalarError>を返す。照合の走査は1回、予約後は確保ゼロ。正規化は行わず、Rustの大小文字変換で単一記号になる候補だけを追加する。

ScalarLimitsの既定はmax_pattern_units=65,535（空白を含むUTF-16長）、max_working_bytes=512 MiB。正の値を要求し、乗算・加算を検査する。コンパイル時は記号・位置の一時配列と完成maskのピーク、実行時はmatcherと渡されたworkspaceの容量を合算する。別matcherで予約したworkspaceも予算を再検査する。呼出側の入力と複数workspace/instanceの合計はこの単体APIでは追跡しない。一括instance全体の予算は§11.6に従いM2以降で管理する。空の語、pattern上限、容量、予算、予約失敗を区別してErrとする。

`Engine`はコーパス単位のRust traitで、関連型Query/Workspace/Errorを持つ。searchは各textの最小level（上限内に無ければNone）を入力順の借用sliceとして返す。QueryとWorkspaceの表現は実装ごとに選べ、逐次エンジンの語幅を後続の転置エンジンへ強制しない。

最初の実装`ScalarEngine<W>`は借用した`&[&str]`全体を走査する。workspace(query,K)で状態語と全text数の出力slotを予約する。query・状態・出力・adapterの容量を合算し、engine/queryの小さい方の予算を適用する。queryのUTF-16単位数もengine側上限を再検査する。他engine由来のworkspaceは出力長と容量予算を検査してから書き込み、失敗時は以前の出力を保持する。返値は同じworkspaceによる次のsearchまで有効で、Rustの借用で同時変更を防ぐ。借用text本体は呼出側所有のためこのadapterの予算から除き、M2の所有索引と区別する。ここではkey順位・正規化・重複排除を行わない。

<a id="section-6"></a>

## 6. Narrowing（候補絞り込み）

<a id="section-6-1"></a>

### 6.1 単調性条件

前回の pattern P の受理位置に自己ループがあり（Include、または WildcardSpace で末尾が空白）、今回の P′ が P の記号列の延長（`P′ = P · s`）なら、任意の text で `cost(P′) ≥ cost(P)`。P′ の任意の整列から、s に当たる部分を P の末尾ループで吸収した P の整列が作れるからである。よって **K′ ≤ K のとき** `{t : cost(P′, t) ≤ K′} ⊆ {t : cost(P, t) ≤ K}`。

- Exact、および末尾ループのない WildcardSpace では成立しないので全量計算する。
- 曖昧度ポリシーで K が増えた打鍵（例: byLength なら 4・8・12 文字目）は全量計算する。
- 後退（Backspace）は候補集合のスタック（上限付き）から復元する。LCP と延長判定は **正規化後・mode の意味を反映した記号列**で行う。内部の末尾 wrap ループを入力の一文字として比較しない。例えば `ｶ` → `ｶﾞ` は正規化後には `か` → `が` という置換であり、延長として Narrowing を適用しない。
- mode、profile、normalizer、K、索引世代もキャッシュ条件に含む。候補数ではなく完全な候補集合を保持し、前回の top-N だけへ絞り込まない。条件を証明できないときは全量へ戻す。
- テスト: 条件成立時は全量計算と結果が一致すること、条件不成立時は絞り込みが使われないこと。

<a id="section-6-2"></a>

### 6.2 Planner の費用モデル

SearchSession の 1 打鍵ごとに、使える選択肢の推定費用を比べて最小のものを選ぶ。

- Narrowing ≈ `c_wm × Σ(候補の記号数)`（§6.1 の条件を満たすときだけ）
- Incremental ≈ `c_row(K_cache) × 総スロット + 抽出費用`（前行がキャッシュにあるときだけ）
- 全量 ≈ `TransposedBlocked(m, K)`（常に可能。キャッシュが無い、LCP が短い、上限超過で捨てた場合など）
- 定数はブラウザごとにベンチで校正して定数ファイルに記録する（値は観測であって保証ではない）。

Rustの`enable_narrowing(index, query, max_bytes)`は全text候補・DP scratch・前回クラス列を予約する。前回末尾に自己ループがあり、mode/leading loop・EQクラスの接頭辞が一致し、Kが増えていない場合だけ候補を再検証する。内部wildcard loopは比較し、移動する末尾受理loopだけは次の文字追加時に取り除ける。top-Nは候補集合へ使わない。候補のUTF-16単位数+境界数の8倍がコーパス語数以下という保守的な暫定費用判定を使う。これは測定済みの普遍的閾値ではなく、M3のブラウザ打鍵校正が残る。stats.narrowed_textsは実際に評価したtext数を返し、K増加・後退・mode変更等ではdenseへ戻る。JS SearchSessionのautoもこの経路を使い、maxCacheBytes=0/engine=transposed-blockedでは無効化する。

---

<a id="section-7"></a>

## 7. 意図表現 DAG（IntentIndex、+α）

<a id="section-7-1"></a>

### 7.1 意図表現の構文

M6のRust/WASM実装はD27の構文・深さ・node/edge・保留状態上限を適用する。正規化/記号化を構文境界で切らず、有限の保留状態を含むDAGへ変換する。v1の照合はDAG×patternの最小費用行列を使い、式が生成する最短/最長記号数から不可能なentryを先に除外する。下記の状態語OR方式は将来の同値最適化候補であり、現行Rust実装の経路と区別する。

- 例: `(保存|更新)(する)?`は、選択肢と省略可能な要素を組み合わせた式。
- v1 で受理するもの: リテラル、`( … )`、`|`、`?`（直前の要素を省略可能にする）、`\` によるエスケープ。`*` と `+` は v1 では拒否する（閉路が生じる。必要になればワークリスト固定点で拡張する）。
- 1 エントリ = 1 つの式 + key（FAQ ID。§20.2 の key と同じ意味）。

<a id="section-7-2"></a>

### 7.2 照合

- 式を文字単位の DAG（ノード番号＝トポロジカル順）に構築する。各ノードに level 0..K の状態語（pattern 位置のビット）を持たせ、辺で 1 記号ぶん遷移し、合流では OR を取る。終端ノードの受理ビットから最小 level を得る。
- 遷移関数は状態集合の和に分配的（各出力ビットが単一の入力ビットの OR で決まる）なので、OR マージは「いずれかの展開で一致」と厳密に等しい（検証済み: 3,000 式・展開文字列 14,903 件を総当たりした結果と不一致 0）。
- 計算量は展開数ではなく DAG の辺数に比例する。re_expand（masui/expand_node、npm `re_expand` 0.2.0）は展開を全列挙しつつ asearch の状態を引き継ぐ方式で、`list.length * srcnode.trans.length < 100000` の条件で展開を黙って打ち切る。本方式ではその打ち切りが要らない。
- Narrowing（§6）はエントリ単位で同じように適用できる。

<a id="section-7-3"></a>

### 7.3 説明（最良展開の復元）

- 上位 N エントリについてだけ、ノードごとの状態語を保持し、受理ビットから後ろ向きに辿る（各ビットは単一の入力ビットに由来するので、一本の経路が必ず存在する）。得られた展開文字列とハイライト位置を返す。

---
