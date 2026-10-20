"""Hypothesis: asearch.ts(t, e) == (ideal_cost(t) <= e) and (e <= m + |t|)  (|t| in iteration units)."""

import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ref import EXACT, INCLUDE, WILDCARD, AsearchTs, compile_ts, ideal_cost

rng = random.Random(int(sys.argv[1]) if len(sys.argv) > 1 else 7)
A = ["a", "b", "c", "A", " ", "漢", "字"]
bad = 0
total = 0
ex = None
for _ in range(40000):
    p = "".join(rng.choice(A) for _ in range(rng.randint(0, 7)))
    mode = rng.choice([EXACT, INCLUDE, WILDCARD])
    t = "".join(rng.choice(A + ["x"]) for _ in range(rng.randint(0, 9)))
    a = AsearchTs(p, mode, unit_iter=False)
    cp = compile_ts(p, mode)
    m = len(cp.syms)
    c = ideal_cost(cp, list(t))
    for e in range(12):
        total += 1
        pred = c <= e <= m + len(t)
        if a.match(t, e) != pred:
            bad += 1
            ex = ex or (p, mode, t, e, c, m)
print("pairs", total, "violations", bad, "example", ex)

# realistic-ish distribution: deviation rate for ambig<=3, texts >= 4 chars
dev = 0
tot = 0
for _ in range(20000):
    p = "".join(rng.choice(A) for _ in range(rng.randint(2, 10)))
    mode = rng.choice([EXACT, INCLUDE, WILDCARD])
    t = "".join(rng.choice(A + ["x"]) for _ in range(rng.randint(4, 40)))
    a = AsearchTs(p, mode, unit_iter=False)
    cp = compile_ts(p, mode)
    c = ideal_cost(cp, list(t))
    for e in range(4):
        tot += 1
        dev += a.match(t, e) != (c <= e)
print("realistic pairs", tot, "deviations", dev)
