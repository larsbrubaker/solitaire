# Winnable-only seed pipeline

This doc covers how the **Options → Winnable deals only** toggles are
wired and how to regenerate the bundled seed lists.

## Variant by variant

| Variant | Seed source | Validation | Bundle file |
| --- | --- | --- | --- |
| **FreeCell** | Microsoft Jim Horne LCG, game numbers 1..32 000 | Don Woods' 1994 sweep — all 32 000 solved except #11982 | (none — algorithm reproduces Microsoft's deal directly) |
| **Spider** | Our `StdRng::seed_from_u64`, validated offline against the 4-suit deck | [Solvitaire](https://github.com/thecharlieblake/Solvitaire) DFS solver, GPL-2.0 | `solitaire-core/assets/spider_winnable_seeds.bin` |
| **Klondike** | Our `StdRng::seed_from_u64`, validated offline | Solvitaire | `solitaire-core/assets/klondike_winnable_seeds.bin` |
| **Mom's Solitaire** | n/a — every deal is winnable by the shuffle mechanic | n/a | n/a |

Each `.bin` file is a flat array of **little-endian `u64` seeds** —
eight bytes per entry, no header, no padding. `solitaire-core` reads
them via `include_bytes!` so growing the bundle is just a matter of
overwriting the file and rebuilding.

A failing test in
[`games/deal_stability_tests.rs`](../solitaire-core/src/games/deal_stability_tests.rs)
catches any drift in the (seed → deal) mapping: if the shuffle ever
changes, the existing bundled seeds stop matching what the engine
actually deals, and the lists need regenerating. Run `cargo test
deal_stability` after any change to a deal function.

## Regenerating the Spider list

1. Install Solvitaire (the published reference is the Docker image
   from the JAIR paper — see `github.com/thecharlieblake/Solvitaire`).
2. Walk seeds `N = 0, 1, 2, …` in Rust:
   ```
   let session = GameSession::new(Spider::four_suit(), N);
   let board_repr = spider_pile_set_to_solvitaire_board(&session.piles);
   ```
   Solvitaire reads a board layout on stdin in its own text format
   (`solitaire --type spider --solvability` with the board piped in).
3. For each seed whose board solves under a wall-clock budget, append
   `N.to_le_bytes()` (8 bytes) to `spider_winnable_seeds.bin`.
4. Re-run `cargo test` — the deal-stability test and the winnable
   loader checks should both pass.

The runtime picker [`games::winnable_seeds::pick_spider_winnable`]
draws uniformly from whatever is in the bundle. The bundle ships
empty; the picker falls through to the wallclock seed in that case,
so the toggle is effectively a no-op until the list is populated.

## Regenerating the Klondike list

Same flow as Spider, with `Klondike::with_draw_count(1)` and a 1-draw
Solvitaire run (`--type klondike-deal-1`). Solvitaire's published
paper measures Klondike winnability at **81.9 %** so the sweep will
discard ~one in five seeds.

We deliberately don't ship a Microsoft Klondike numbering — modern
Microsoft Solitaire Collection daily challenges aren't a public
enumerable list, and the original Win 3.1..Win 7 Klondike never
exposed game numbers. Players reproduce a specific deal by sharing
the raw `u64` seed (visible via `GameSession::seed()`).

## FreeCell: no solver needed

`solitaire-core::games::ms_freecell::deal_columns(n)` reproduces the
exact deal Jim Horne's Windows FreeCell produces for game number
`n`. The toggle picks an `n ∈ [1, 32_000] \ {11982}` and stores it
in the session's `seed` field as a `u64`, so the player can recall
the same Microsoft game later by typing the same number.

Microsoft Solitaire Collection later extended the range to
1..1 000 000 with the same algorithm; we cap at 32 000 because
that's where comprehensive third-party winnability data ends.
