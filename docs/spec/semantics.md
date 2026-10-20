# 照合意味論

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-2"></a>

## 2. 参照意味論（asearch.ts@1.0.3）

<a id="section-2-1"></a>

### 2.1 公開 API 面（検証済み: npm `asearch.ts@1.0.3` の `lib/index.js` と `lib/index.d.ts`）

- exports: `MatchMode`（数値 enum。実行時は逆引き付きオブジェクト `{"0":"Exact","1":"Include","2":"WildcardSpace","Exact":0,"Include":1,"WildcardSpace":2}`）と `Asearch`。
- `class Asearch { static MAXLENGTH = 31; readonly matchMode; readonly pattern; masks: {[char: string]: number}; constructor(pattern: string, mode?: MatchMode); match(query: string, ambig?: number): boolean }`
- 型の上で private: `acceptPattern`, `initPattern`, `loopMask`, `verbose()`。実行時の状態フィールドは列挙可能な自前プロパティ（`Object.keys` → `loopMask, masks, matchMode, pattern, initPattern, acceptPattern`）。`verbose()`はprototype上のメソッドで、自前プロパティには含まれない。
- `masks` の値は符号付き int32（位置 0 のビットは `-2147483648`）。
- `package.json`: `main = lib/index.js`（CommonJS、TypeScript の ES5 ターゲット出力）、`types = lib/index.d.ts`。

<a id="section-2-2"></a>

### 2.2 パターンのコンパイル（lib と src で同一）

```text
if (pattern.length > 31) throw new Error(`Pattern must be shorter than 32 chars. But given ${pattern}`)
      // pattern.length は UTF-16 長で空白も数える
splitted = Array.from(pattern)          // code point 単位
INIT   = 1 << 31（符号なしとして扱う）
accept = INIT >>> splitted.length
loop   = (mode === Include) ? (INIT | accept) : 0
pos = 0
for c of splitted:
  if (mode === WildcardSpace && c === ' ') { loop |= INIT >>> pos; accept <<= 1; continue }
  for key of [c, c.toLocaleLowerCase(), c.toUpperCase()]: masks[key] |= INIT >>> pos
  pos++
```

- Include は位置 0 と受理位置にループ（＝部分一致）。WildcardSpace は空白の直前位置にループ（空白は位置を消費しない＝`.*`）。Exact と未知の mode はループなし。
- 大文字小文字は **pattern 側だけ**最大 3 キーに展開し、text 側は加工しない。複数記号になるキー（`'ß'.toUpperCase() === 'SS'` 等）は単一記号と一致しないので実質無視される。

<a id="section-2-3"></a>

### 2.3 1 記号ずつの遷移

```text
states[e] = INIT >>> e        for e = 0..ambig       // JS のシフト量は mod 32
for each symbol x of text:
  mk = masks[x] ?? 0
  for e = ambig .. 1:
    states[e] = (states[e] & loop) | ((states[e] & mk) >>> 1) | (states[e-1] >>> 1) | states[e-1]
  states[0] = ((states[0] & mk) >>> 1) | (states[0] & loop)
  for e = 1 .. ambig:
    states[e] |= states[e-1] >>> 1
return (states[ambig] & accept) !== 0
```

下位 level の更新前の値（挿入・置換）と、更新後の値（削除の閉包）を使い分ける点に注意。

<a id="section-2-4"></a>

### 2.4 記号化の差（重要）

- TypeScript ソース（`for (const char of query)`）は code point 単位で走査する。
- **npm 公開物 `lib/index.js` は ES5 への降格で `for (i = 0; i < query.length; i++) char = query[i]`、つまり UTF-16 code unit 単位で走査する**（検証済み: `tools/reference/check_lib_quirks.js`）。pattern 側は `Array.from` でcode point単位のため、Exact・level 0では同じ絵文字同士でも一致しない。textに非BMPが含まれるだけで常に不一致になるわけではなく、例えばIncludeでpatternが`a`、textが`a😀`ならlevel 0で一致する。
  - 実測: `new Asearch('a😀b').match('a😀b', 0) === false`、`match('a😀b', 2) === true`、`new Asearch('😀').match('😀') === false`。
  - README の「絵文字対応」は公開物では成立していない。

<a id="section-2-5"></a>

### 2.5 実測で確認した癖（npm `asearch.ts@1.0.3`、Node v22.22.0）

| ID  | 現象                                                                                                                                                      | 本プロジェクトの扱い                                                                          |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Q1  | text を UTF-16 単位で走査（§2.4）                                                                                                                         | 既定プロファイル `AsearchTs` は code point（意図的逸脱）。ビット一致が必要なら `AsearchTsLib` |
| Q2  | 長さ検査は UTF-16 長（空白含む）。絵文字 16 個（32 単位）で例外                                                                                           | compat API は同一メッセージで完全再現。拡張 API は 31 記号制限なし（§11.6 の資源上限は適用）  |
| Q3  | ambig に対して非単調。pattern `"a"` vs `""` → e=0..3 で `[F,T,F,F]`、`"ab"` vs `""` → `[F,F,T,F,F]`                                                       | compat API は完全再現。一括 API は最小 level を返すので影響なし（§2.8）                       |
| Q4  | ambig ≥ 32 でシフト量が折り返す（`"abc"` vs `"abc"`: ambig=31 → false、32 → true）                                                                        | compat API は u32 とシフト量 mod 32 で完全再現                                                |
| Q5  | ambig が非整数（1.5）・負（-1）→ false                                                                                                                    | 再現                                                                                          |
| Q6  | `match()` が毎回 `process.env.ASEARCH_VERBOSE` を参照し、`process` の無い環境（ポリフィル無しのブラウザ）で `ReferenceError`（検証済み: vm コンテキスト） | **再現しない**。Node 専用グローバルの参照を全面禁止                                           |
| Q7  | 未知の mode（3 など）は Exact として振る舞う                                                                                                              | 再現                                                                                          |
| Q8  | `toLocaleLowerCase` は実行環境のロケールに依存（tr/az/lt 等）                                                                                             | compat では候補キーを JS 側で同じ式で計算して WASM に渡し、完全一致させる（§12.2）            |
| Q9  | 引数なしの `match()` は TypeError                                                                                                                         | TypeError であれば良い（メッセージ一致は不要）                                                |
| Q10 | 文字列でない pattern（数値）は空パターン相当で動く                                                                                                        | 再現しない（TypeError）。逸脱として文書化                                                     |

<a id="section-2-6"></a>

### 2.6 プロファイル

| Profile               | text 記号   | pattern 記号 | 境界 B(i,e)        | 大文字小文字                                    | 備考                                                            |
| --------------------- | ----------- | ------------ | ------------------ | ----------------------------------------------- | --------------------------------------------------------------- |
| `AsearchTs`（既定）   | code point  | code point   | `i == e`           | JS から渡す候補キー（既定 `[c, lower, upper]`） | ソースの意味論＝README の意図                                   |
| `AsearchTsLib`        | UTF-16 unit | code point   | `i == e`           | 同上                                            | npm 公開物とビット一致                                          |
| `NodeAsearch`（任意） | UTF-16 unit | UTF-16 unit  | `i == 0 && e == 0` | ASCII のみ                                      | 空白は常にワイルドカード、K ≤ 3、非空白 32 記号以上は常に false |
| `Ideal`               | code point  | code point   | `i <= e`           | 設定可                                          | 正しい初期 ε 閉包（Levenshtein）                                |

B(i,e) は「text を 1 記号も読む前に、level e の状態集合に位置 i が含まれるか」。

<a id="section-2-7"></a>

### 2.7 理想コスト（Ideal）

```text
C[i][0] = i                                                    (i = 0..m)
C[i][j] = min( C[i][j-1]   + (i ∈ LOOPS ? 0 : 1),               // ループ / 挿入
               C[i-1][j-1] + (EQ_i(t_j) ? 0 : 1),               // 一致 / 置換
               C[i-1][j]   + 1 )                                // 削除
cost(p, t) = C[m][n]
```

<a id="section-2-8"></a>

### 2.8 特性定理（検証済み）

**定理.** `AsearchTs` と `AsearchTsLib` で、任意の pattern p・mode・text t・整数 `0 ≤ e ≤ 31` について

```text
asearch.ts.match(t, e)  ⇔  cost(p, t) ≤ e  ∧  e ≤ m + n
```

（記号化はプロファイルに従う。m は非ワイルドカード記号数、n は text の記号数。）

**系.** `cost ≤ m + n` は常に成立する（全削除＋全挿入）。したがって「`match` が最初に true になる e」は、K 以内なら常に `cost` に等しい。**最小 level の一覧（＝ランキング）は `AsearchTs` と `Ideal` で完全に一致する。** 非単調性（Q3）は cost より上の level にしか現れない。

**証明スケッチ.** asearch.ts の状態集合を L_e(t) とすると、帰納法で `L_e(t) = {i : C[i][t] ≤ e} ∩ [e − t, ∞)` が示せる。初期値は `L_e(0) = {e}`。位置は単調非減少にしか動かず、下限 `e − t` より下の位置 i は `i + t < e`、すなわち `C[i][t] ≤ i + t ≤ e − 1` を満たすので、同じ遷移先は level e−1 からの挿入・置換・削除で必ず再生される。受理位置 m については `m ≥ e − n ⇔ e ≤ m + n`。

**検証記録.** `characterize.py`（2 シード計 96 万三つ組で反例 0）、`first_level.py`（12 万ペアで不一致 0）、`verify.py`（npm 実物と 2 シード計 20,071 ペア × level 0..5 で一致）。

**設計上の帰結（MUST）.**

1. 最小 level を求める評価（転置、絞り込み、DAG、Myers、および ScalarWm の一括用途）は **Ideal（単調）**で行う。ScalarWm は compat 用に asearch.ts の初期状態も持つ（初期状態の定数を切り替えるだけ）。
2. compat の `Asearch.match(t, e)` は asearch.ts の u32 レプリカ（§5）で実装する。テストでは `level(t) ≤ e ∧ e ≤ m + n` と突き合わせる。
3. `matchLevel` と一括 API は最小 level を返すので、プロファイル差（`AsearchTs` と `Ideal`）は記号化と大文字小文字の扱いだけになる。

---
