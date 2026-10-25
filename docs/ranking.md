# Result ranking

[日本語](ja/ranking.md)

The default immutable-index order remains `(minimum edit distance, order, key)`. A document is returned once, using its best matching text. ASIX v1 bytes and matching acceptance do not change.

With `QueryOptions::rank_by_position = true` (0.3+), the order is `(minimum edit distance, earliest match start, order, key)`. Starts are zero-based offsets in normalized UTF-16. A match beginning at offset 1 ranks before offset 2 at the same distance; an exact match still ranks before an earlier approximate match. Among aliases, the minimum distance wins, then the earliest start, then input text order. An alignment that consumes no symbol has `usize::MAX` as its start and ranks last within its distance.

The matcher first determines accepted texts and their minimum distances. A rolling dynamic-programming row then finds the earliest start for matching aliases that can still improve their document, including aliases outside the requested top-N. An alias cannot win if its distance is worse than the current best, or if the distance is equal and the current start is already zero. Those aliases skip positional DP but still enter the complete candidate history used by later queries. Literal, substitution and paid insertion transitions consume the match span; free Include/wildcard loops do not. The whole candidate set is sorted before truncation. Live segment results merge by the same order, so delta and compacted views agree. Start ranking is separate from the explanation's deterministic highlighting tie-breaks; it does not change those spans.

The row is reserved with the workspace and charged to the existing working-memory budget; no unbounded traceback matrix is allocated. Positional ranking adds work proportional to the lengths of matching texts times literal query length, plus candidate sorting. It is opt-in to preserve existing ordering and performance. Incremental/narrowing caches still select accepted candidates; ranking is recomputed from the complete matches on every query.

The WASM search-config ABI accepts the previous seven words (stable order), or eight words with a final boolean flag: `0` keeps stable order and `1` selects positional order. Other lengths or flag values are rejected. Positional packed results append one start per hit after the ordinary hits and totals; `0xffffffff` represents no consumed symbol. Stable packed results retain their existing layout. The demo's TypeScript adapter exposes `ranking: "position"` and optional `SearchResult.starts`; result copies preserve starts and shard merging requires a consistent ranking mode. Intent DAG searches retain their own existing ordering and do not accept this option.

A fixed ceiling of 64 × 1,024 × 1,024 positional transitions is shared across the whole segment column per search. Each segment charges the upper bound `normalized UTF-16 text length × compiled literal count` for every live matching alias before its positional DP. This upper-bound charge still includes aliases whose positional DP can be skipped, so pruning does not relax the guard. Excess fails with RESOURCE_LIMIT instead of returning partially ranked results. Limit-zero totals skip positional work, and an empty literal pattern consumes no DP transitions. This CPU-work guard is separate from the configurable memory budget.

Session cache planning reserves the widest positional result transport layout before the first query configuration is known, so checkpoints cannot consume that result headroom.

A positional-work rejection invalidates the affected segment's planner and checkpoints. The next query, including an extension of the rejected prefix, recomputes complete candidates without requiring a session reset. No partial candidate history is reused.

Stable Rust searches leave SearchHit.start uncomputed at usize::MAX; the TypeScript adapter omits starts entirely in stable mode.
