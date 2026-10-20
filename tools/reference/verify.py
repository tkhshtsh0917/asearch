"""Differential verification:
1. Python replicas  == real npm packages (asearch.ts@1.0.3 lib, asearch@1.0.1)
2. transposed DP (profile) == replica, for every text and every level e <= K (bit-exact)
3. transposed DP (ideal)   == ideal_cost <= e
4. statistics: how often asearch.ts deviates from the ideal semantics
"""

import json
import os
import random
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ref import (
    EXACT,
    INCLUDE,
    WILDCARD,
    AsearchTs,
    NodeAsearch,
    compile_node,
    compile_ts,
    ideal_cost,
    transposed,
    utf16_len,
    utf16_units,
)

NM = sys.argv[1]
SEED = int(sys.argv[2]) if len(sys.argv) > 2 else 1
N_CASES = int(sys.argv[3]) if len(sys.argv) > 3 else 3000
rng = random.Random(SEED)

PAT_ALPHA = ["a", "b", "c", "A", "B", " ", "漢", "字", "😀", "ß"]
TXT_ALPHA = ["a", "b", "c", "A", "B", " ", "漢", "字", "😀", "x", "\ud83d", "SS"[0]]


def rand_str(alpha, lo, hi):
    return "".join(rng.choice(alpha) for _ in range(rng.randint(lo, hi)))


def gen_case():
    r = rng.random()
    if r < 0.05:
        p = ""
    elif r < 0.10:
        p = " " * rng.randint(1, 3)
    elif r < 0.15:  # near the 31-unit limit (incl. surrogate pairs)
        p = rand_str(["a", "b", "漢", "😀", " "], 28, 33)
    else:
        p = rand_str(PAT_ALPHA, 1, 8)
    mode = rng.choice([EXACT, INCLUDE, WILDCARD])
    texts = []
    for _ in range(rng.randint(1, 6)):
        if rng.random() < 0.15:
            texts.append("")
        elif rng.random() < 0.3 and p:
            # mutate the pattern: near-miss texts exercise the error levels
            t = list(p)
            for _ in range(rng.randint(0, 3)):
                op = rng.random()
                j = rng.randint(0, len(t))
                if op < 0.33 and t:
                    del t[min(j, len(t) - 1)]
                elif op < 0.66:
                    t.insert(j, rng.choice(TXT_ALPHA))
                elif t:
                    t[min(j, len(t) - 1)] = rng.choice(TXT_ALPHA)
            if rng.random() < 0.3:
                t = (
                    list(rand_str(TXT_ALPHA, 0, 3))
                    + t
                    + list(rand_str(TXT_ALPHA, 0, 3))
                )
            texts.append("".join(t))
        else:
            texts.append(rand_str(TXT_ALPHA, 0, 12))
    ks = list(range(6))
    return {"p": p, "mode": mode, "texts": texts, "ks": ks}


cases = [gen_case() for _ in range(N_CASES)]
proc = subprocess.run(
    ["node", os.path.join(os.path.dirname(__file__), "oracle.js"), NM],
    input=json.dumps(cases),
    capture_output=True,
    text=True,
    check=True,
)
oracle = json.loads(proc.stdout)

K = 5
stats = {
    "cases": 0,
    "pairs": 0,
    "replica_ts_mismatch": 0,
    "replica_node_mismatch": 0,
    "trans_ts_mismatch": 0,
    "trans_ts_src_mismatch": 0,
    "trans_node_mismatch": 0,
    "trans_ideal_mismatch": 0,
    "ts_vs_ideal_diff_pairs": 0,
    "ts_vs_ideal_diff_k_le3": 0,
    "ts_nonmonotone_pairs": 0,
    "throws": 0,
}
examples = {}

for c, o in zip(cases, oracle):
    stats["cases"] += 1
    p, mode, texts, ks = c["p"], c["mode"], c["texts"], c["ks"]
    # ---- asearch.ts
    if o["tsErr"] is not None:
        stats["throws"] += 1
        try:
            AsearchTs(p, mode)
            stats["replica_ts_mismatch"] += 1
            examples.setdefault("replica_ts_throw", (p, o["tsErr"]))
        except ValueError:
            assert utf16_len(p) > 31
    else:
        a_lib = AsearchTs(p, mode, unit_iter=True)
        a_src = AsearchTs(p, mode, unit_iter=False)
        cp = compile_ts(p, mode)
        tr_lib = transposed(cp, [utf16_units(t) for t in texts], K, "asearch_ts")
        tr_src = transposed(cp, [list(t) for t in texts], K, "asearch_ts")
        tr_ideal = transposed(cp, [list(t) for t in texts], K, "ideal")
        for ti, t in enumerate(texts):
            stats["pairs"] += 1
            rep = [a_lib.match(t, k) for k in ks]
            if rep != o["ts"][ti]:
                stats["replica_ts_mismatch"] += 1
                examples.setdefault("replica_ts", (p, mode, t, rep, o["ts"][ti]))
            if tr_lib[ti] != rep[: K + 1]:
                stats["trans_ts_mismatch"] += 1
                examples.setdefault("trans_ts", (p, mode, t, tr_lib[ti], rep))
            rep_src = [a_src.match(t, k) for k in ks]
            if tr_src[ti] != rep_src[: K + 1]:
                stats["trans_ts_src_mismatch"] += 1
                examples.setdefault("trans_ts_src", (p, mode, t, tr_src[ti], rep_src))
            cost = ideal_cost(cp, list(t))
            ideal = [cost <= k for k in range(K + 1)]
            if tr_ideal[ti] != ideal:
                stats["trans_ideal_mismatch"] += 1
                examples.setdefault("trans_ideal", (p, mode, t, tr_ideal[ti], cost))
            if rep_src[: K + 1] != ideal:
                stats["ts_vs_ideal_diff_pairs"] += 1
                if rep_src[:4] != ideal[:4]:
                    stats["ts_vs_ideal_diff_k_le3"] += 1
                    examples.setdefault(
                        "ts_vs_ideal_k_le3", (p, mode, t, rep_src[:4], cost)
                    )
            if any(rep_src[k] and not rep_src[k + 1] for k in range(K)):
                stats["ts_nonmonotone_pairs"] += 1
                examples.setdefault("ts_nonmonotone", (p, mode, t, rep_src))
    # ---- node-asearch (spaces are always wildcards, ambig capped at 3)
    nm_ = NodeAsearch(p)
    cpn = compile_node(p)
    trn = transposed(cpn, [utf16_units(t) for t in texts], 3, "node")
    for ti, t in enumerate(texts):
        rep = [nm_.match(t, k) for k in ks]
        if rep != o["node"][ti]:
            stats["replica_node_mismatch"] += 1
            examples.setdefault("replica_node", (p, t, rep, o["node"][ti]))
        if len(cpn.syms) <= 31 and trn[ti] != rep[:4]:
            stats["trans_node_mismatch"] += 1
            examples.setdefault("trans_node", (p, t, trn[ti], rep))

print(json.dumps(stats, ensure_ascii=False, indent=1))
for k, v in examples.items():
    print(k, "=>", json.dumps(v, ensure_ascii=False))
