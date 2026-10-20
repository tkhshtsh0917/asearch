"""Reference models used to validate the AGENTS.md specification.

- AsearchTs      : bit-exact replica of asearch.ts@1.0.3 (published ES5 lib, unit_iter=True)
                   or of its TypeScript source semantics (unit_iter=False, code points).
- NodeAsearch    : bit-exact replica of node-asearch (npm "asearch") 1.0.1.
- ideal_cost     : textbook DP (Levenshtein + 0-cost self loops) with the full initial
                   epsilon-closure C[i][0] = i  ("ideal" semantics).
- transposed     : the proposed text-parallel bit-parallel DP (bits = corpus positions,
                   one row per pattern position, one bitmap per error level), with a
                   pluggable boundary column so that it can emulate every variant.
"""
from typing import Callable, Dict, List, Sequence

M32 = 0xFFFFFFFF
EXACT, INCLUDE, WILDCARD = 0, 1, 2


def utf16_units(s: str) -> List[str]:
    out = []
    for ch in s:
        cp = ord(ch)
        if cp >= 0x10000:
            v = cp - 0x10000
            out.append(chr(0xD800 + (v >> 10)))
            out.append(chr(0xDC00 + (v & 0x3FF)))
        else:
            out.append(ch)
    return out


def utf16_len(s: str) -> int:
    return len(utf16_units(s))


def ushr(x: int, n: int) -> int:  # JS: x >>> n
    return (x & M32) >> (n & 31)


def shl32(x: int, n: int) -> int:  # JS: x << n (bit pattern)
    return ((x & M32) << (n & 31)) & M32


class AsearchTs:
    MAXLENGTH = 31

    def __init__(self, pattern: str, mode: int = EXACT, unit_iter: bool = True):
        if utf16_len(pattern) > self.MAXLENGTH:
            raise ValueError(f"Pattern must be shorter than 32 chars. But given {pattern}")
        self.unit_iter = unit_iter
        splitted = list(pattern)  # Array.from(pattern): code points
        left = 0x80000000
        self.init = left
        self.accept = ushr(left, len(splitted))
        self.loop = (self.init | self.accept) if mode == INCLUDE else 0
        self.masks: Dict[str, int] = {}
        pos = 0
        for c in splitted:
            if mode == WILDCARD and c == " ":
                self.loop |= ushr(left, pos)
                self.accept = shl32(self.accept, 1)
                continue
            for key in (c, c.lower(), c.upper()):
                self.masks[key] = self.masks.get(key, 0) | ushr(left, pos)
            pos += 1

    def match(self, text: str, ambig=0) -> bool:
        if not isinstance(ambig, int) or ambig < 0:
            return False
        states = [ushr(self.init, i) for i in range(ambig + 1)]
        seq = utf16_units(text) if self.unit_iter else list(text)
        for ch in seq:
            m = self.masks.get(ch, 0)
            for i in range(len(states) - 1, 0, -1):
                states[i] = ((states[i] & self.loop) | ushr(states[i] & m, 1)
                             | ushr(states[i - 1], 1) | states[i - 1])
            states[0] = ushr(states[0] & m, 1) | (states[0] & self.loop)
            for i in range(1, len(states)):
                states[i] |= ushr(states[i - 1], 1)
        return (states[ambig] & self.accept) != 0


def _toupper(c: int) -> int:
    return c - 0x20 if 0x61 <= c <= 0x7A else c


def _tolower(c: int) -> int:
    return c + 0x20 if 0x41 <= c <= 0x5A else c


class NodeAsearch:
    def __init__(self, source: str):
        mask = 0x80000000
        self.shift: Dict[int, int] = {}
        self.eps = 0
        for u in utf16_units(source):
            c = ord(u)
            if c == 0x20:
                self.eps |= mask
            else:
                for k in (c, _toupper(c), _tolower(c)):
                    self.shift[k] = self.shift.get(k, 0) | mask
                mask >>= 1
        self.accept = mask

    def match(self, s: str, ambig: int = 0) -> bool:
        i0, i1, i2, i3 = 0x80000000, 0, 0, 0
        e = self.eps
        for u in utf16_units(s):
            m = self.shift.get(ord(u), 0)
            i3 = (i3 & e) | ((i3 & m) >> 1) | (i2 >> 1) | i2
            i2 = (i2 & e) | ((i2 & m) >> 1) | (i1 >> 1) | i1
            i1 = (i1 & e) | ((i1 & m) >> 1) | (i0 >> 1) | i0
            i0 = (i0 & e) | ((i0 & m) >> 1)
            i1 |= i0 >> 1
            i2 |= i1 >> 1
            i3 |= i2 >> 1
        if ambig >= 4:
            ambig = 3
        return ([i0, i1, i2, i3][ambig] & self.accept) != 0


# ---------------------------------------------------------------- compiled pattern
class Compiled:
    """Pattern compiled to (symbols, loop positions, equality predicate)."""

    def __init__(self, syms: List[str], loops: set, eq: Callable[[str, str], bool]):
        self.syms = syms
        self.loops = loops
        self.eq = eq


def compile_ts(pattern: str, mode: int) -> Compiled:
    syms, loops, pos = [], set(), 0
    for c in pattern:  # code points
        if mode == WILDCARD and c == " ":
            loops.add(pos)
            continue
        syms.append(c)
        pos += 1
    if mode == INCLUDE:
        loops |= {0, len(syms)}

    def eq(p: str, t: str) -> bool:
        return t == p or t == p.lower() or t == p.upper()

    return Compiled(syms, loops, eq)


def compile_node(pattern: str) -> Compiled:
    syms, loops, pos = [], set(), 0
    for u in utf16_units(pattern):
        if u == " ":
            loops.add(pos)
            continue
        syms.append(u)
        pos += 1

    def eq(p: str, t: str) -> bool:
        c = ord(p)
        return ord(t) in (c, _toupper(c), _tolower(c))

    return Compiled(syms, loops, eq)


def ideal_cost(cp: Compiled, text: Sequence[str]) -> int:
    m = len(cp.syms)
    col = list(range(m + 1))  # full initial epsilon-closure
    for t in text:
        new = [0] * (m + 1)
        for i in range(m + 1):
            best = col[i] + (0 if i in cp.loops else 1)
            if i > 0:
                best = min(best, col[i - 1] + (0 if cp.eq(cp.syms[i - 1], t) else 1))
                best = min(best, new[i - 1] + 1)
            new[i] = best
        col = new
    return col[m]


# ---------------------------------------------------------------- transposed DP
INIT_PROFILES = {
    "ideal": lambda i, e: i <= e,       # C[i][0] = i
    "asearch_ts": lambda i, e: i == e,  # states[e] = INIT >>> e
    "node": lambda i, e: i == 0 and e == 0,  # INITSTATE = [INIT,0,0,0]
}


class Layout:
    """Corpus layout: [B][text0][B][text1]... one boundary slot B before every text."""

    def __init__(self, texts: List[List[str]]):
        pos, sep, textmask = 0, 0, 0
        occ: Dict[str, int] = {}
        ends = []
        for t in texts:
            sep |= 1 << pos
            pos += 1
            for ch in t:
                occ[ch] = occ.get(ch, 0) | (1 << pos)
                textmask |= 1 << pos
                pos += 1
            ends.append(pos - 1)  # == boundary slot for an empty text
        self.n = pos
        self.all = (1 << pos) - 1
        self.sep = sep
        self.textmask = textmask
        self.occ = occ
        self.ends = ends

    def shift(self, x: int) -> int:
        return (x << 1) & self.all

    def fill(self, x: int) -> int:
        """Segmented prefix-OR (0-cost self loop): propagate every set bit to the end of its text.
        Boundary bits seed the first text position. Uses the carry trick ((y+M)^M)&M | y."""
        M = self.textmask
        y = (x & M) | (self.shift(x & self.sep) & M)
        f = (((y + M) ^ M) & M) | y
        return f | (x & self.sep)


def eq_bitmap(cp: Compiled, lay: Layout, i: int) -> int:
    p = cp.syms[i - 1]
    r = 0
    for s, bm in lay.occ.items():
        if cp.eq(p, s):
            r |= bm
    return r


def row_step(cp: Compiled, lay: Layout, prev: List[int], i: int, K: int, init) -> List[int]:
    """Compute row i (pattern prefix length i) for all levels 0..K from row i-1."""
    eqi = eq_bitmap(cp, lay, i) if i > 0 else 0
    cur: List[int] = []
    for e in range(K + 1):
        if i == 0:
            x = lay.shift(cur[e - 1]) if e > 0 else 0          # insertion only
        else:
            x = lay.shift(prev[e]) & eqi                       # match
            if e > 0:
                x |= (lay.shift(prev[e - 1])                   # substitution
                      | prev[e - 1]                            # deletion (same column)
                      | lay.shift(cur[e - 1]))                 # insertion
        x &= ~lay.sep & lay.all                                # boundary column = init
        if init(i, e):
            x |= lay.sep
        if i in cp.loops:
            x = lay.fill(x)                                    # 0-cost self loop
        cur.append(x)
    return cur


def transposed_rows(cp: Compiled, lay: Layout, K: int, profile: str) -> List[List[int]]:
    init = INIT_PROFILES[profile]
    rows = [row_step(cp, lay, [], 0, K, init)]
    for i in range(1, len(cp.syms) + 1):
        rows.append(row_step(cp, lay, rows[-1], i, K, init))
    return rows


def accept_matrix(lay: Layout, last_row: List[int]) -> List[List[bool]]:
    """acc[text][e] = True iff the text matches at level e."""
    return [[bool((last_row[e] >> end) & 1) for e in range(len(last_row))] for end in lay.ends]


def transposed(cp: Compiled, texts: List[List[str]], K: int, profile: str) -> List[List[bool]]:
    lay = Layout(texts)
    rows = transposed_rows(cp, lay, K, profile)
    return accept_matrix(lay, rows[-1])
