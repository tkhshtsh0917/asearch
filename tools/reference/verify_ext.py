"""Extensions:
(1) DAG matcher for intent expressions "(A|B)C?" WITHOUT expansion:
    OR-merge of asearch.ts per-level state words over a topologically ordered DAG must equal
    min over all expansions of the asearch.ts min-level.
(2) Damerau (OSA) transposition in the transposed DP (ideal profile) vs naive OSA DP.
(3) Incremental row reuse == full recompute (append chars, including wildcard appends).
"""

import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ref import (
    EXACT,
    INCLUDE,
    WILDCARD,
    AsearchTs,
    Compiled,
    Layout,
    accept_matrix,  # noqa
    compile_ts,
    row_step,
    INIT_PROFILES,
    ushr,
)

rng = random.Random(5)

# ---------------------------------------------------------------- (1) DAG
# expression AST: ('lit', str) | ('cat', [..]) | ('alt', [..]) | ('opt', node)


def gen_expr(depth=0):
    r = rng.random()
    if depth > 2 or r < 0.35:
        return ("lit", "".join(rng.choice("abcAB漢") for _ in range(rng.randint(1, 3))))
    if r < 0.6:
        return ("cat", [gen_expr(depth + 1) for _ in range(rng.randint(2, 3))])
    if r < 0.9:
        return ("alt", [gen_expr(depth + 1) for _ in range(rng.randint(2, 3))])
    return ("opt", gen_expr(depth + 1))


def expand(e):
    k = e[0]
    if k == "lit":
        return {e[1]}
    if k == "cat":
        out = {""}
        for c in e[1]:
            out = {a + b for a in out for b in expand(c)}
        return out
    if k == "alt":
        return set().union(*[expand(c) for c in e[1]])
    return expand(e[1]) | {""}


class Graph:
    """Character-level DAG: node ids are created in topological order; edges carry one char or eps."""

    def __init__(self):
        self.n = 0
        self.edges = []  # (u, v, ch or None)

    def node(self):
        self.n += 1
        return self.n - 1

    def build(self, e, s):
        """returns end node reachable from start s"""
        k = e[0]
        if k == "lit":
            cur = s
            for ch in e[1]:
                v = self.node()
                self.edges.append((cur, v, ch))
                cur = v
            return cur
        if k == "cat":
            cur = s
            for c in e[1]:
                cur = self.build(c, cur)
            return cur
        if k == "alt":
            end_nodes = [self.build(c, s) for c in e[1]]
            j = self.node()
            for t in end_nodes:
                self.edges.append((t, j, None))
            return j
        t = self.build(e[1], s)  # opt
        j = self.node()
        self.edges.append((t, j, None))
        self.edges.append((s, j, None))
        return j


def ts_step(a: AsearchTs, states, ch):
    states = list(states)
    m = a.masks.get(ch, 0)
    for i in range(len(states) - 1, 0, -1):
        states[i] = (
            (states[i] & a.loop)
            | ushr(states[i] & m, 1)
            | ushr(states[i - 1], 1)
            | states[i - 1]
        )
    states[0] = ushr(states[0] & m, 1) | (states[0] & a.loop)
    for i in range(1, len(states)):
        states[i] |= ushr(states[i - 1], 1)
    return states


def dag_min_level(a: AsearchTs, expr, K):
    g = Graph()
    s = g.node()
    end = g.build(expr, s)
    B = [None] * g.n
    B[s] = [ushr(a.init, e) for e in range(K + 1)]
    # nodes were created in topological order: process edges grouped by source in id order
    out = {}
    for u, v, ch in g.edges:
        out.setdefault(u, []).append((v, ch))
    for u in range(g.n):
        if B[u] is None:
            continue
        for v, ch in out.get(u, []):
            nb = B[u] if ch is None else ts_step(a, B[u], ch)
            B[v] = nb if B[v] is None else [x | y for x, y in zip(B[v], nb)]
    if B[end] is None:
        return 255
    for e in range(K + 1):
        if B[end][e] & a.accept:
            return e
    return 255


bad = 0
tested = 0
total_expansions = 0
for _ in range(3000):
    expr = gen_expr()
    strs = expand(expr)
    if len(strs) > 400:
        continue
    tested += 1
    total_expansions += len(strs)
    p = "".join(rng.choice("abcAB漢 ") for _ in range(rng.randint(1, 5)))
    mode = rng.choice([EXACT, INCLUDE, WILDCARD])
    a = AsearchTs(p, mode, unit_iter=False)
    K = 3
    brute = 255
    for s_ in strs:
        for e in range(K + 1):
            if a.match(s_, e):
                brute = min(brute, e)
                break
    # NOTE: asearch.ts is non-monotone in ambig; "first e that matches" is what both compute
    got = dag_min_level(a, expr, K)
    if got != brute:
        bad += 1
        if bad < 3:
            print("DAG mismatch", expr, p, mode, got, brute)
print(
    "DAG vs expansion: expressions",
    tested,
    "expanded strings",
    total_expansions,
    "mismatches",
    bad,
)

# ---------------------------------------------------------------- (2) OSA in transposed DP


def osa_cost(cp: Compiled, text):
    m, n = len(cp.syms), len(text)
    INF = 10**9
    D = [[INF] * (n + 1) for _ in range(m + 1)]
    for i in range(m + 1):
        D[i][0] = i
    for j in range(1, n + 1):
        for i in range(m + 1):
            best = D[i][j - 1] + (0 if i in cp.loops else 1)
            if i > 0:
                best = min(
                    best,
                    D[i - 1][j - 1] + (0 if cp.eq(cp.syms[i - 1], text[j - 1]) else 1),
                )
                best = min(best, D[i - 1][j] + 1)
            if (
                i > 1
                and j > 1
                and cp.eq(cp.syms[i - 1], text[j - 2])
                and cp.eq(cp.syms[i - 2], text[j - 1])
            ):
                best = min(best, D[i - 2][j - 2] + 1)
            D[i][j] = best
    return D[m][n]


def transposed_osa(cp: Compiled, texts, K):
    lay = Layout(texts)
    init = INIT_PROFILES["ideal"]
    from ref import eq_bitmap

    rows = [row_step(cp, lay, [], 0, K, init)]
    eqs = [0] + [eq_bitmap(cp, lay, i) for i in range(1, len(cp.syms) + 1)]
    for i in range(1, len(cp.syms) + 1):
        cur = row_step(cp, lay, rows[-1], i, K, init)
        if i >= 2:
            # transposition: S^e_i |= shift2(S^{e-1}_{i-2}) & shift1(EQ_i) & EQ_{i-1}
            # (text[j-1]==p_i and text[j]==p_{i-1}); both chars must be text chars (not boundary)
            new = []
            for e in range(K + 1):
                x = cur[e]
                if e > 0:
                    t = (
                        lay.shift(lay.shift(rows[i - 2][e - 1]))
                        & lay.shift(eqs[i])
                        & eqs[i - 1]
                    )
                    x |= t
                    # insertion term must see the updated lower level: recompute dependent part
                new.append(x)
            # re-close levels upward (insertion/deletion terms depend on lower levels of the same row)
            fixed = []
            for e in range(K + 1):
                x = new[e]
                if e > 0:
                    x |= lay.shift(fixed[e - 1])  # insertion from updated lower level
                x = (x & ~lay.sep & lay.all) | (lay.sep if init(i, e) else 0)
                if i in cp.loops:
                    x = lay.fill(x)
                fixed.append(x)
            cur = fixed
        rows.append(cur)
    return accept_matrix(lay, rows[-1])


bad = 0
tot = 0
for _ in range(3000):
    p = "".join(rng.choice("abcd ") for _ in range(rng.randint(1, 6)))
    mode = rng.choice([EXACT, INCLUDE, WILDCARD])
    cp = compile_ts(p, mode)
    texts = [
        "".join(rng.choice("abcdx") for _ in range(rng.randint(0, 8))) for _ in range(4)
    ]
    acc = transposed_osa(cp, [list(t) for t in texts], 3)
    for t, row in zip(texts, acc):
        tot += 1
        c = osa_cost(cp, list(t))
        if row != [c <= e for e in range(4)]:
            bad += 1
            if bad < 3:
                print("OSA mismatch", p, mode, t, row, c)
print("OSA pairs", tot, "mismatches", bad)

# ---------------------------------------------------------------- (3) incremental == full
bad = 0
for _ in range(500):
    p = "".join(rng.choice("abAB漢 ") for _ in range(rng.randint(1, 7)))
    mode = WILDCARD
    texts = [
        list("".join(rng.choice("abAB漢x ") for _ in range(rng.randint(0, 9))))
        for _ in range(5)
    ]
    lay = Layout(texts)
    init = INIT_PROFILES["asearch_ts"]
    # incremental: build rows prefix by prefix; a space applies fill to the current last row
    rows = [row_step(compile_ts("", mode), lay, [], 0, 3, init)]
    prefix = ""
    for ch in p:
        prefix += ch
        cp = compile_ts(prefix, mode)
        if ch == " ":
            rows[-1] = [lay.fill(x) for x in rows[-1]]
        else:
            rows.append(row_step(cp, lay, rows[-1], len(cp.syms), 3, init))
        full = __import__("ref").transposed_rows(cp, lay, 3, "asearch_ts")
        if full[-1] != rows[-1]:
            bad += 1
print("incremental vs full mismatches:", bad)
