# Spider solver — clean-room implementation spec

**Provenance**: derived from Blake & Gent, "The Winnability of Klondike
Solitaire and Many Other Patience Games", JAIR Vol. 85, Feb 2026
(arXiv:1906.12314v5) and first-principles reasoning about Spider's
rules. **No source code from `thecharlieblake/Solvitaire` was
consulted.** This document records which design decisions came from
the paper and which are novel (and therefore the implementer's
responsibility to justify).

License: MIT, matching the rest of `solitaire-core`. Clean-room
re-implementation deliberately avoids GPL contamination from the
Solvitaire source tree.

---

## 1. Architecture: DFS + transposition table + canonical hash

**Paper § 5.1** — exhaustive depth-first backtracking, trailing instead
of state-copy. Single mutable `PileSet`; each move recorded with
enough info to revert exactly. This matches our existing
apply/revert structure in `games/spider_solver.rs`.

**Paper § 5.2** — every visited state cached in a transposition table.
LRU eviction. Never discard ancestors of the current state. Compressed
key (not the full state) — paper records each pile component's card
ordering plus face-up/face-down flag per card.

Our implementation: cache key is a `u64` hash of the canonical
serialised state (see § 2). LRU is approximated by capacity-bounded
`HashSet<u64>` plus a per-frame insertion guard that bypasses the
ancestor-discard problem because every ancestor state is already in
the stack — even if it's evicted from the TT, we don't re-explore it
because the DFS frame holds the next-move cursor.

**Novel decisions**:
- 64-bit hash, not full state — accept rare collisions in exchange for
  RAM. Use FxHash or xxHash3 (final pick during implementation).
- Cache capacity: 100 M entries default (paper Table 5 uses 100 M for
  Klondike at ~8 GB RAM). For Spider scale TBD by benchmark.

## 2. Canonical state serialisation

**Paper § 5.3** — "Before storing states in a cache we reduce them to a
canonical form, maintaining each group of indistinguishable locations
such as tableau piles and free cells in a sorted order. For efficiency
this order is maintained incrementally during search."

For Spider:
- 10 tableau piles are mutually interchangeable → sort the 10 piles by
  some total order before hashing.
- 8 foundation piles are mutually interchangeable, but each is either
  empty or holds exactly one completed K-A run. Reduce to a single
  scalar = count of completed-suit foundations (0..8).
- Stock is order-sensitive — paint top-of-stock to bottom-of-stock as
  a flat sequence in original deal order.

**Novel: tableau-pile comparator.** Paper silent. Choices:
- (a) Lexicographic over `(card_byte, ...)` where `card_byte` packs
  rank, suit and face-up bit.
- (b) Length-first, then lexicographic.

Pick (a) — simpler, no observable difference in TT hit rate at
expected pile depths.

**Novel: card-byte encoding** for hashing. 8-bit:
```
bit 7    : face_up (0 = face-down, 1 = face-up)
bits 6-5 : suit (0=♣, 1=♦, 2=♥, 3=♠)
bits 4-0 : rank (1=A .. 13=K, 0 reserved for sentinel)
```
Pure choice — paper doesn't dictate. Documented here so the comparator
is reproducible.

## 3. Dominances

**Paper Theorem 1 (D1, safe auto-foundation builds)**: Appendix C.1.
*Excluded from Spider* by paper § 5.4.3 — "disabled completely for
games of more than one deck, for games with Spider-type building rules,
or games like Gaps without either foundations or a hole".

**Paper Theorem 4 (D2, partial-pile move restriction)**: Appendix C.2.
*Excluded from Spider* by paper § C.2:

> "Some games, such as Spider, use a policy where individual cards can
> be moved in any suit but built groups can only be moved if they are
> all the same suit. This means that moves of groups can be necessary
> to establish sequences of the same suit, even if the card above is
> not buildable."

→ Neither formally-proved dominance from the paper applies to Spider.

**Spider-specific prunes (novel, first-principles)**:

### P1 — Singleton-from-pile-of-1 to empty pile
Move that takes the sole card off pile A and drops it onto empty pile
B is a no-op modulo pile-order canonicalisation: the new state is
identical to the old under § 2's pile-sort. Skip such moves at
generation time.

### P2 — Pure suit-shuffle prune
A single-card or run move that both (a) does not flip a face-down,
(b) creates a same-suit junction with the destination's top, and
(c) destroys an identical same-suit junction at the source's predecessor.
This is a net-zero exchange under TT canonicalisation. Skip.

(Already in our existing `generate_choices` — keep.)

### P3 — Atomic deal-row
Spider's "deal from stock" lays one card on each of the 10 cascades in
one user action. Our generator emits a single `Choice = Vec<Move>` of
length 10 for the deal, not 10 separate Choice options. This is
already correct in the current implementation and is critical for not
exploring nonsensical partial-deal states.

### P4 — Complete K-A run never broken
Once a K-A suited run sits at the top of a cascade, Spider's rules
force its immediate collapse to a foundation (single deterministic
move). The solver applies this as a forced collapse after every move,
inside the apply step (same pattern as our existing
`collapse_step`), so it never appears as a Choice.

**Open question**: is P4 a true dominance for proving unwinnability? Yes
— forcing a forced move can only narrow the search; if a deal is
solvable with the collapse, it's still solvable with the collapse, and
no solvable deal requires *not* collapsing.

## 4. Streamliner: suit symmetry (S2)

**Paper § 5.5**:

> "If we have a position that differs from a previously visited state
> only in suits (but not in colours) in the tableau, it is very
> unlikely to succeed if the first one does not."

Implemented as a streamlined hash that collapses suit detail before
caching. Streamliner = optimistic: may produce false negatives (report
unwinnable when game is actually winnable). Wrapped in a "smart"
re-search (see § 5).

**Spider variants**:

| Variant   | Suit-symmetry collapse                              |
|-----------|------------------------------------------------------|
| 1-suit    | All 4 suits collapse to one symbol (all cards same suit anyway in real 1-suit). |
| 2-suit    | Collapse to two symbols by colour (red=♦+♥, black=♣+♠). |
| 4-suit    | No collapse — paper's example targeted red-black builders; for Spider's any-suit build policy the paper notes most "differs only in suits" hops are unproductive but doesn't formally extend S2 to 4-suit. We skip the streamliner for 4-suit. |

**Novel: bit-mask collapse implementation.** In the streamlined hash
path, replace the suit bits of `card_byte` (§ 2) with the
colour-class mapping. Rank + face-up bits unchanged.

## 5. Smart-streamliner wrapper

**Paper § 5.5**: "smart streamliner" allocates 10 % of the time budget
to a streamlined search; if that fails, restart with 100 % of the full
budget under the strict solver.

For Spider 1-suit / 2-suit: smart wrapper is the default. For 4-suit:
skip; run the strict solver directly (the streamliner doesn't help).

Single time-budget knob, derived ratios.

## 6. Move generation order

**Paper § 5.1.1**: "We use a very naive approach to create the list of
possible legal moves at each node in search... we do not optimise
checking which rules apply".

**Novel: heuristic ordering** (clean-room improvement; paper
explicitly didn't bother).

Generate Choices in priority order, push to the DFS stack so highest
priority pops first:

| Score | Choice                                       |
|-------|----------------------------------------------|
| 5000  | Move that exposes a face-down tableau card.  |
| 1500  | Move that completes a K-A suited run (immediate foundation collapse). |
| 500   | Move that creates a same-suit junction (extends a suited run). |
| 100   | Move that places a King on an empty column.  |
| 50 * take | Multi-card moves with longer tails (bias toward consolidating). |
| 10    | Plain alt-rank move (no junction change).    |
| 1     | Deal-row from stock (last resort).           |

Ordering is a search heuristic — DOES NOT affect correctness of
exhaustive search (DFS still explores every branch on backtrack), but
strongly affects time-to-first-solution.

## 7. Random instance generation — IGNORED for clean-room

**Paper § 8.2**: Mersenne Twister `mt19937` from `<random>`, seed
incremented every 52 random numbers, applied as a Fisher-Yates shuffle
of the deck.

We do **not** replicate Solvitaire's RNG. Our solver consumes a
canonical 104-card permutation directly (`PileSet` post-deal). For
oracle validation (§ 8) we parse the **deal column** of Solvitaire's
published CSVs and ignore their seed semantics. Our own seed scheme
remains `u64 → StdRng → Fisher-Yates`, unchanged.

## 8. Oracle validation — statistical equivalence

License: CC0 (figshare metadata confirmed).

**Original plan revised after inspecting the data:** Solvitaire's
published CSVs in `ExperimentalResults/spider/v0.08/` contain only
the seed integer + verdict + stats. The actual 104-card deal is
**not** serialised — Solvitaire reconstructs it from the seed by
running its own `mt19937 → Fisher-Yates` shuffle. To replay
seed-equivalent deals would require either:
- (a) Reading Solvitaire's deal-generation source code (violates the
  clean-room rule), or
- (b) Re-deriving the recipe from prose hints in the paper
  (under-specified — paper says "initial seed incremented after every
  52 random numbers" but Spider is a 2-deck game so it's unclear
  whether the increment is every 52 or every 104).

We therefore drop the seed-equivalent oracle in favour of a
**population-level winnability check**:

1. Bulk-classify 10,000 fresh `StdRng`-generated Spider 4-suit
   deals using our clean-room solver at the same per-deal budget
   Solvitaire used (4 hour, 200M cache).
2. Compute the 95 % Wilson confidence interval for our winnability
   estimate.
3. **Acceptance**: our CI overlaps Solvitaire's published
   **98.487 % ± 1.513 %** (paper Table 1, Spider row).

If our CI sits materially below Solvitaire's, our solver is missing
something — do not ship until investigated. Same protocol applies
to 1-suit and 2-suit (paper's reported numbers + CIs).

This is weaker than per-seed equivalence but still defensible: two
independent solvers agreeing on population winnability is what the
paper itself used to validate Solvitaire against existing
specialised solvers (paper § 7, comparing against Birrell, Wolter,
Fish on first-N seeds).

## 9. Module structure

Rust:

```
solitaire-core/src/games/
├── spider.rs                  // rules, layout, legality check
├── spider_solver.rs           // DELETE old version; replace with...
├── spider/
│   ├── mod.rs
│   ├── state.rs               // canonical PileSet helpers + serialisation
│   ├── search.rs              // DFS + TT
│   ├── prunes.rs              // P1..P4
│   ├── streamliner.rs         // S2 collapse + smart wrapper
│   ├── moves.rs               // Choice generation + heuristic ordering
│   └── tests.rs               // unit tests + oracle harness
```

Cap each file ≤ 600 lines.

## 10. Validation gates

Each phase must pass before the next:

1. **Unit**: synthetic positions with known verdicts (pre-won board,
   locked board, single-move-to-win, etc.). Existing tests in
   `spider_solver` survive.

2. **Oracle**: 100 % match on 1,000 Solvitaire-classified deals from
   the Figshare CSVs across all three Spider suit-counts.

3. **Bulk**: classify 10,000 fresh `StdRng` seeds at the 5-minute
   timeout. Report per-variant winnability percentage. Target:
   - 1-suit: ≈ 100 %
   - 2-suit: ≈ 95 %
   - 4-suit: ≈ 98.5 % (matches paper's 98.487 % ± 1.513 %)

If 4-suit comes in materially under 98 %, our solver is missing
something; do not ship.

## 11. License + commit plan

All new code MIT (header inheriting from `solitaire-core/Cargo.toml`).
First commit cites this SPEC at the top of every file:

```rust
//! Clean-room Spider solver — see
//! `solitaire-core/src/games/SPEC_SPIDER_SOLVER.md`. Algorithms
//! derived from the JAIR paper alone; no Solvitaire source consulted.
```
