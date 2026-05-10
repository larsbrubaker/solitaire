//! Markdown content for the Help dialogs (About + per-variant rules).
//! Kept separate from the dialog widget so future edits to copy don't
//! force a recompile of the rendering code.

use super::app_model::HelpKind;

pub fn title_for(kind: HelpKind) -> &'static str {
    match kind {
        HelpKind::About => "About Solitaire",
        HelpKind::Klondike => "Klondike — Rules",
        HelpKind::FreeCell => "FreeCell — Rules",
        HelpKind::Spider => "Spider — Rules",
    }
}

pub fn markdown_for(kind: HelpKind) -> &'static str {
    match kind {
        HelpKind::About => ABOUT,
        HelpKind::Klondike => KLONDIKE,
        HelpKind::FreeCell => FREECELL,
        HelpKind::Spider => SPIDER,
    }
}

const ABOUT: &str = r#"
# Solitaire

A small, fast, scaling-clean implementation of the classic card games:
Klondike (1- and 3-card draw), FreeCell, and Spider — with more on the
way.

## Built on

- **agg-gui** — a Rust GUI library that pairs an AGG-derived 2D
  rasteriser with a wgpu/winit backend, so card art, text, and
  widget chrome render through a single pixel-accurate path.
- **CC0 SVG playing cards** — Loren Osborn's *English pattern playing
  cards deck PLUS CC0* on Wikimedia Commons (built on Dmitry Fomin's
  card faces, with extra contributions from Guy vandegrift). Released
  under CC0 1.0 Universal — no attribution required, but credited
  here gladly.

## A note on people

This game exists because of programmers who shared the craft.

In 1989 my cousin **Marlin Eller** wrote a solitaire game in Forth
on a Mac+ over a couple of long evenings, as a Mother's Day gift for
his mom **Margaret Eller**. I sat next to him and watched him write
the entire thing — for more than eight hours across two days. At
some point he turned to me and said, *"no one who doesn't love
programming can sit here and watch someone write code."* I'm a
programmer because of that gift. Margaret got hers; I got mine.

A future release of this app will include Marlin's original variant.

## Source

Available under MIT/Apache-2.0 at
**github.com/larsbrubaker/solitaire**.
"#;

const KLONDIKE: &str = r#"
# Klondike

The classic Solitaire most people grew up with. Sometimes called
"Patience" or, when the Microsoft 3-card-draw flavour is meant,
just "Solitaire."

## Layout

- **7 tableau columns**, dealt 1, 2, 3, 4, 5, 6, 7 cards. Only the
  topmost card of each column is face-up.
- **4 foundations** (top-right) — empty at start.
- **Stock** (top-left) — the remaining 24 cards, face-down.
- **Waste** — sits next to the stock; cards drawn from the stock land
  here face-up.

## How to play

- **Tableau → tableau**: drop one or more cards onto another tableau
  column if the receiving top card is **one rank higher** and **the
  opposite colour** (red on black, black on red).
- **Tableau / Waste → foundation**: drop a single card onto a
  foundation pile if it's an Ace (to start the pile) or one rank
  **higher** in the **same suit**.
- **Empty tableau column**: only a King (or a King-led run) may be
  placed on an empty column.
- **Stock click**: deals **1 card** to the waste in standard mode, or
  **3 cards** in 3-draw mode (set via Options → Draw 3). When the
  stock is empty, clicking it recycles the waste back to the stock
  face-down.
- **Auto-foundation**: double-click any topmost face-up card to send
  it to the first foundation that legally accepts it.

## Win condition

All 52 cards on the four foundations, ordered Ace → King in their
own suit.

## Tips

- Reveal face-down tableau cards as early as possible — covered
  cards are dead weight until uncovered.
- Don't rush an Ace or 2 to the foundation if it's the only card
  blocking a productive tableau move.
- 3-card draw is meaningfully harder than 1-card; only every third
  draw is reachable, so plan further ahead.
"#;

const FREECELL: &str = r#"
# FreeCell

A near-fully-skill-based variant — almost every deal is winnable, and
all cards are visible from the start.

## Layout

- **8 cascade columns**, dealt left-to-right with 7, 7, 7, 7, 6, 6, 6,
  6 cards. All cards face-up.
- **4 free cells** (top-left) — temporary parking spots, each holds
  one card.
- **4 foundations** (top-right) — empty at start, build Ace → King by
  suit.

## How to play

- **Cascade → cascade**: drop a card onto another cascade if the
  receiving top is **one rank higher** and **opposite colour**.
- **Cascade → free cell**: park any topmost card in any empty free
  cell.
- **Cascade / Free cell → foundation**: drop a single card onto a
  foundation if it's an Ace, or one rank higher in the same suit.
- **Free cell → cascade**: same colour-and-rank rules as cascade →
  cascade.
- **Multi-card moves**: dragging a run of N cards is allowed only if
  you have enough free cells + empty cascades to perform the move
  one card at a time. The engine enforces this automatically.

## Win condition

All 52 cards on the foundations, Ace → King by suit.

## Tips

- Treat free cells as scarce. Filling all four is usually a trap.
- Empty cascades are even more valuable than free cells — they
  multiply the size of the runs you can move.
- Don't bury low cards behind high ones early; you'll need to dig
  them out again to start the foundations.
"#;

const SPIDER: &str = r#"
# Spider

Two decks, four suits, more cards than tableau width — a long,
patient game where you build complete K → A runs.

## Layout

- **10 cascade columns**, dealt 6, 6, 6, 6, 5, 5, 5, 5, 5, 5 cards.
  Topmost card of each column is face-up; the rest face-down.
- **8 foundations** — accept a single complete K-down-to-A run in a
  single suit. They auto-collapse from the cascade when complete.
- **Stock** — 50 cards, dealt 10 at a time across the 10 cascades.

## How to play

- **Cascade → cascade**: drop one or more cards onto another cascade
  if the receiving top is **one rank higher** (suits don't matter
  for stacking, but only **same-suit** runs can be moved as a unit).
- **Stock click**: deals one card face-up to each of the 10 cascades.
  Requires no cascade is empty.
- **Complete-run auto-foundation**: a run of 13 same-suit cards
  K-to-A in a cascade automatically moves to a foundation slot.

## Win condition

All 8 foundations filled with K → A runs (104 cards total, 8 sets).

## Tips

- Suit matters for movement, not for stacking. A 5♠ on a 6♥ is
  legal — but you can only move that pair as a unit if it's same-
  suit.
- Don't deal from the stock until you've made every possible move
  on the board. Each stock-deal lands face-up cards on top of every
  cascade and can bury work in progress.
"#;
