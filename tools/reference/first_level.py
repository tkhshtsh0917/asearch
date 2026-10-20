"""Corollary: first e in 0..K with AsearchTs.match(t,e) == ideal_cost if ideal_cost <= K else none.
(because ideal_cost <= m + n always holds: delete all pattern symbols + insert all text symbols)"""

import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ref import EXACT, INCLUDE, WILDCARD, AsearchTs, compile_ts, ideal_cost, utf16_units

rng = random.Random(99)
A = ["a", "b", "c", "A", " ", "漢", "字", "😀"]
bad = tot = 0
for _ in range(60000):
    p = "".join(rng.choice(A) for _ in range(rng.randint(0, 8)))
    mode = rng.choice([EXACT, INCLUDE, WILDCARD])
    t = "".join(rng.choice(A + ["x"]) for _ in range(rng.randint(0, 10)))
    for unit in (False, True):
        a = AsearchTs(p, mode, unit_iter=unit)
        cp = compile_ts(p, mode)
        seq = utf16_units(t) if unit else list(t)
        c = ideal_cost(cp, seq)
        K = 5
        first = next((e for e in range(K + 1) if a.match(t, e)), None)
        exp = c if c <= K else None
        tot += 1
        if first != exp:
            bad += 1
            if bad < 4:
                print("counterexample", p, mode, t, unit, first, exp)
        assert c <= len(cp.syms) + len(seq)
print("pairs", tot, "first-level mismatches", bad)
