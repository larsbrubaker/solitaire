//! Per-game Help content (Rules + About) rendered through agg-gui's
//! `MarkdownView`. Kept separate from the dialog widget so future
//! edits to copy don't force a recompile of the rendering code. Each
//! game owns BOTH its rules and its about — only Mom's mentions
//! Marlin and Margaret, since that variant exists because of them.

use crate::games::GameKind;

use super::app_model::HelpKind;

pub fn title_for(kind: HelpKind) -> &'static str {
    match kind {
        HelpKind::Rules(GameKind::Klondike) => "Klondike — Rules",
        HelpKind::Rules(GameKind::FreeCell) => "FreeCell — Rules",
        HelpKind::Rules(GameKind::Spider) => "Spider — Rules",
        HelpKind::Rules(GameKind::MomsSolitaire) => "Mom's Solitaire — Rules",
        HelpKind::About(GameKind::Klondike) => "About Klondike",
        HelpKind::About(GameKind::FreeCell) => "About FreeCell",
        HelpKind::About(GameKind::Spider) => "About Spider",
        HelpKind::About(GameKind::MomsSolitaire) => "About Mom's Solitaire",
        HelpKind::AboutSuite => "About",
    }
}

pub fn markdown_for(kind: HelpKind) -> &'static str {
    match kind {
        HelpKind::Rules(GameKind::Klondike) => KLONDIKE_RULES,
        HelpKind::Rules(GameKind::FreeCell) => FREECELL_RULES,
        HelpKind::Rules(GameKind::Spider) => SPIDER_RULES,
        HelpKind::Rules(GameKind::MomsSolitaire) => MOMS_RULES,
        HelpKind::About(GameKind::Klondike) => KLONDIKE_ABOUT,
        HelpKind::About(GameKind::FreeCell) => FREECELL_ABOUT,
        HelpKind::About(GameKind::Spider) => SPIDER_ABOUT,
        HelpKind::About(GameKind::MomsSolitaire) => MOMS_ABOUT,
        HelpKind::AboutSuite => SUITE_ABOUT,
    }
}

// ────────────────────────────────────────────────────────────────────
// Rules
// ────────────────────────────────────────────────────────────────────

const KLONDIKE_RULES: &str = r#"
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
"#;

const FREECELL_RULES: &str = r#"
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
"#;

const SPIDER_RULES: &str = r#"
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
"#;

const MOMS_RULES: &str = r#"
# Mom's Solitaire

Mom's Solitaire is a Montana / Gaps variant. There's no stock, no
waste, no foundations — just every card laid out face-up on a
**13 × 4 grid**, with the four Aces serving as moveable gaps. Cards
play to the **right**, building same-suit runs from King down to 2.

## Layout

- **13 columns × 4 rows = 52 cells**, one card per cell, all face-up.
- The four Aces are the **gaps**. They aren't movable — they're the
  empty space you slide cards into.

## How to play

Mom's is **click-only** — there's no dragging. Click on a gap and
the game finds the card that fits and swaps it in.

- **A gap in column 1..12:** click it; the game looks at the card
  immediately to the **left** of the gap and pulls in that left
  neighbour's **same-suit, one-rank-lower partner** from wherever
  it currently sits. (Left neighbour 6♥ → 5♥ jumps in. Left
  neighbour 5♣ → 4♣. And so on.)
- **A gap in column 0** (the leftmost): only a **King** can fill it,
  and you pick which suit. Click the col-0 gap to **arm** it (the
  game waits), then click any **King** anywhere on the board — that
  King swaps in and its suit fixes the row's target colour.
- **An Ace can't be the source of a move.** It's not a card you
  pick up; it's the gap you fill.
- **Stuck gaps:** if the card to the left of a gap is a 2 or
  another Ace, no card fits — that gap is dead until adjacent moves
  free it.

## Win condition

Each row's columns 0..11 form a **same-suit K → 2 run** (column 12
ends up holding that row's Ace). All four rows in this state means
the deck is sorted — you've won.

## A note on the name

This game is named for a Mother's Day 1989 gift my cousin Marlin
Eller wrote in Forth on a Mac+ for his mom Margaret. See **Help →
About** for the story.
"#;

// ────────────────────────────────────────────────────────────────────
// About — per-game. Marlin and Margaret appear ONLY in Mom's.
// Shared engine credits live at the bottom of every variant's About.
// ────────────────────────────────────────────────────────────────────

const SHARED_CREDITS: &str = r#"
## About this implementation

- **agg-gui** — the Rust GUI library this app is built on.
- **CC0 SVG playing cards** — Loren Osborn's *English pattern playing
  cards deck PLUS CC0* on Wikimedia Commons (built on Dmitry Fomin's
  card faces, with extra contributions from Guy vandegrift). Released
  under CC0 1.0 Universal — no attribution required, but credited
  here gladly.
- Source: [github.com/larsbrubaker/solitaire](https://github.com/larsbrubaker/solitaire).
"#;

const KLONDIKE_ABOUT: &str = const_concat::concat!(
    r#"
# About Klondike

Klondike is the solitaire most people just call "Solitaire." It
takes its name from the Klondike Gold Rush of 1896–99, where the
game became a popular pastime among prospectors. It belongs to the
broader **Patience** family of single-player card games that emerged
in northern Europe in the late 18th century, but Klondike's
particular layout — seven cascading columns and four foundations —
is the version that travelled with miners across the Yukon and
eventually showed up bundled with Microsoft Windows in 1990.

The 3-card-draw flavour shipped as the default in early Microsoft
Solitaire and is what most people remember from those office
afternoons; this app exposes both 1- and 3-card draw as a setting
under **Options → Draw 1 / Draw 3**.
"#,
    SHARED_CREDITS
);

const FREECELL_ABOUT: &str = const_concat::concat!(
    r#"
# About FreeCell

FreeCell as we know it was created by **Paul Alfille** in 1978 on
the **PLATO** computer system at the University of Illinois. Alfille
built on an older variant called *Eight Off* but added the eight
cascades and the rule that almost every deal is winnable — a sharp
break from Klondike, where roughly one deal in three is a dead end
from the start.

It went mainstream when Microsoft bundled it with **Windows 3.1's
Entertainment Pack** in 1991 and then with every copy of Windows
from 95 onward. Among the 32,000 numbered "Microsoft FreeCell"
deals, only one — game **#11982** — has been proven unwinnable.
"#,
    SHARED_CREDITS
);

const SPIDER_ABOUT: &str = const_concat::concat!(
    r#"
# About Spider

Spider is older than its modern reputation suggests — printed
references date it to at least the 1940s, and it was a fixture of
patience compendiums long before Microsoft's **Windows ME** and
**Windows XP Plus!** packs put it in front of millions of office
workers. Its name is usually traced to its eight foundations: four
pairs of legs.

The 1-suit and 2-suit difficulties are common simplifications;
this app currently plays the **4-suit** version (the hardest), where
suit matters for assembling K → A runs onto the foundations.
"#,
    SHARED_CREDITS
);

const MOMS_ABOUT: &str = const_concat::concat!(
    r#"
# About Mom's Solitaire

In 1989, my cousin **Marlin Eller** wrote a solitaire game in Forth
on a Mac+ over a couple of long evenings, as a Mother's Day gift for
his mom **Margaret Eller**. I sat next to him and watched him write
the entire thing — for more than eight hours across two days. At
some point he turned to me and said, *"no one who doesn't love
programming can sit here and watch someone write code."* I'm a
programmer because of that gift. Margaret got hers; I got mine.

The game is a **Montana / Gaps** variant — all 52 cards face-up in
a 13×4 grid, the four Aces as gaps, runs built same-suit K → 2 from
left to right. The rules in this Rust app are ported from a later
C# / agg-sharp re-implementation of the original Forth program
(under `MatterCAD/Submodules/agg-sharp/examples/MomsSolitaire/`).

See **Help → Rules** for how to play.
"#,
    SHARED_CREDITS
);

const SUITE_ABOUT: &str = const_concat::concat!(
    r#"
# About

This Solitaire suite is published by **OneAndDone.games** — a tiny
studio that builds polished, no-tracking, no-ads desktop and web
games for people who just want to play. One purchase, you own it,
done. Hence the name.

The suite bundles four classic Patience variants — **Klondike**,
**FreeCell**, **Spider**, and **Mom's Solitaire** — sharing one
card-rendering pipeline, one event loop, and one set of game-
agnostic services (undo, settings persistence, hints). Pick a
variant from the title screen; the menu bar's **Game** and
**Options** entries adapt to whichever game is in play.

## Winnable deals only

Every variant ships with an Options-menu toggle **Winnable deals
only**, on by default. With the toggle on:

- **FreeCell** uses Microsoft's classic Jim Horne game numbers
  (1..1,000,000), drawing the canonical deal for each. Eight of
  the million — including the famous #11982 — are unwinnable and
  filtered out. Players who know a favourite Microsoft FreeCell
  game number can replay the exact deal they remember.
- **Spider** and **Klondike** draw from bundled lists of seeds
  whose deals have been verified by a depth-first solver. Each
  active deal shows its identifier in the HUD next to a small ✓
  glyph; trade deal numbers with friends to compare strategies on
  the same board.

Turning the toggle off opens the random pool — every shuffle is
possible, including the genuinely unwinnable ones.
"#,
    SHARED_CREDITS
);

// Tiny compile-time string concat — the shared credits block is
// reused at the foot of every variant's About. Pulled into a private
// module so the trait import doesn't pollute the file's surface.
mod const_concat {
    /// `concat!` only takes literal expressions; this macro lets us
    /// glue together two `&'static str` constants at compile time.
    macro_rules! concat {
        ($a:expr, $b:expr) => {{
            const A: &str = $a;
            const B: &str = $b;
            const LEN: usize = A.len() + B.len();
            const fn build() -> [u8; LEN] {
                let mut out = [0u8; LEN];
                let a = A.as_bytes();
                let b = B.as_bytes();
                let mut i = 0;
                while i < a.len() {
                    out[i] = a[i];
                    i += 1;
                }
                let mut j = 0;
                while j < b.len() {
                    out[a.len() + j] = b[j];
                    j += 1;
                }
                out
            }
            const BYTES: &[u8] = &build();
            // SAFETY: BYTES is the concatenation of two valid &str
            // byte slices; the result is therefore valid UTF-8.
            const RESULT: &str = unsafe { std::str::from_utf8_unchecked(BYTES) };
            RESULT
        }};
    }

    pub(super) use concat;
}
